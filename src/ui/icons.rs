//! Presentation-only icons. Never invoke an action, generate thumbnails, or block search.
use super::{Launcher, Row};
use slint::Model;
use spotlight_prototype::{
    discovery::Com,
    model::{Action, Item},
};
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    thread,
};
use windows::{
    Win32::{Foundation::SIZE, Graphics::Gdi::*, System::Registry::*, UI::Shell::*},
    core::HSTRING,
};

#[derive(Clone)]
struct Pixels {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}
#[derive(Default)]
struct Work {
    request: Option<(u64, Vec<Item>)>,
    stop: bool,
}
#[derive(Clone)]
pub struct Requests(Arc<(Mutex<Work>, Condvar)>);
pub struct Loader {
    pub requests: Requests,
    join: Option<thread::JoinHandle<()>>,
}
impl Requests {
    pub fn send(&self, generation: u64, items: Vec<Item>) {
        let (lock, cv) = &*self.0;
        lock.lock().unwrap().request = Some((generation, items));
        cv.notify_one();
    }
}
impl Loader {
    pub fn start(ui: slint::Weak<Launcher>, generation: Arc<AtomicU64>) -> Self {
        let requests = Requests(Arc::new((Mutex::new(Work::default()), Condvar::new())));
        let shared = requests.clone();
        let join = thread::spawn(move || {
            let Ok(_com) = Com::new() else {
                return;
            };
            let mut cache: BTreeMap<String, Option<Pixels>> = BTreeMap::new();
            loop {
                let (number, items) = {
                    let (lock, cv) = &*shared.0;
                    let mut work = lock.lock().unwrap();
                    while !work.stop && work.request.is_none() {
                        work = cv.wait(work).unwrap();
                    }
                    if work.stop {
                        break;
                    }
                    work.request.take().unwrap()
                };
                for (index, item) in items.into_iter().take(6).enumerate() {
                    if generation.load(Ordering::SeqCst) != number
                        || shared.0.0.lock().unwrap().stop
                    {
                        break;
                    }
                    if !cache.contains_key(&item.id) {
                        if cache.len() >= 128
                            && let Some(key) = cache.keys().next().cloned()
                        {
                            cache.remove(&key);
                        }
                        cache.insert(item.id.clone(), load(&item));
                    }
                    let Some(pixels) = cache.get(&item.id).cloned().flatten() else {
                        continue;
                    };
                    let weak = ui.clone();
                    let generation = generation.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if generation.load(Ordering::SeqCst) != number {
                            return;
                        }
                        let Some(ui) = weak.upgrade() else {
                            return;
                        };
                        let model = ui.get_rows();
                        if let Some(model) = model.as_any().downcast_ref::<slint::VecModel<Row>>()
                            && let Some(mut row) = model.row_data(index)
                            && row.id.as_str() == item.id
                        {
                            let buffer =
                                slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
                                    &pixels.rgba,
                                    pixels.width,
                                    pixels.height,
                                );
                            row.icon = slint::Image::from_rgba8(buffer);
                            model.set_row_data(index, row);
                        }
                    });
                }
                spotlight_prototype::diagnostics::record("icon_batch_finished", cache.len() as u64);
            }
        });
        Self {
            requests,
            join: Some(join),
        }
    }
}
impl Drop for Loader {
    fn drop(&mut self) {
        let (lock, cv) = &*self.requests.0;
        lock.lock().unwrap().stop = true;
        cv.notify_one();
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

fn load(item: &Item) -> Option<Pixels> {
    if !item.available {
        return None;
    }
    let path = match &item.action {
        Action::Shortcut { path, .. } | Action::Folder(path) | Action::Document(path) => {
            path.to_string_lossy().into_owned()
        }
        Action::Packaged { aumid } => format!("shell:AppsFolder\\{aumid}"),
        Action::Steam { app_id, .. } => steam_icon(*app_id)?,
        _ => return None,
    };
    unsafe {
        let factory: IShellItemImageFactory =
            SHCreateItemFromParsingName(&HSTRING::from(path), None).ok()?;
        // ICONONLY avoids reading document content or requesting file thumbnails.
        let bitmap = factory
            .GetImage(SIZE { cx: 32, cy: 32 }, SIIGBF_ICONONLY)
            .ok()?;
        let pixels = bitmap_pixels(bitmap);
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        pixels
    }
}
fn steam_icon(id: u32) -> Option<String> {
    // Installed Steam title metadata often supplies its cached game icon here.
    // No game-directory traversal and no changes to the discovery index.
    for key in [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER] {
        let sub = HSTRING::from(format!(
            "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\Steam App {id}"
        ));
        let mut value = [0u16; 4096];
        let mut size = (value.len() * 2) as u32;
        let result = unsafe {
            RegGetValueW(
                key,
                &sub,
                windows::core::w!("DisplayIcon"),
                RRF_RT_REG_SZ,
                None,
                Some(value.as_mut_ptr().cast()),
                Some(&mut size),
            )
        };
        if result.is_ok() {
            let end = value.iter().position(|c| *c == 0).unwrap_or(value.len());
            let path = String::from_utf16_lossy(&value[..end]);
            // Common DisplayIcon values are a quoted path with optional resource index.
            let path = path
                .rsplit_once(',')
                .filter(|(_, suffix)| suffix.trim().parse::<i32>().is_ok())
                .map(|(p, _)| p)
                .unwrap_or(&path)
                .trim()
                .trim_matches('"');
            let path = std::path::Path::new(path);
            if spotlight_prototype::discovery::local_path(path) {
                return Some(path.to_string_lossy().into_owned());
            }
        }
    }
    None
}
unsafe fn bitmap_pixels(bitmap: HBITMAP) -> Option<Pixels> {
    unsafe {
        let mut info = BITMAP::default();
        if GetObjectW(
            HGDIOBJ(bitmap.0),
            std::mem::size_of::<BITMAP>() as i32,
            Some((&mut info as *mut BITMAP).cast()),
        ) == 0
        {
            return None;
        }
        let (width, height) = (info.bmWidth, info.bmHeight.abs());
        if !(1..=128).contains(&width) || !(1..=128).contains(&height) {
            return None;
        }
        let mut buffer = vec![0u8; width as usize * height as usize * 4];
        let mut header = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let dc = CreateCompatibleDC(None);
        if dc.0.is_null() {
            return None;
        }
        let lines = GetDIBits(
            dc,
            bitmap,
            0,
            height as u32,
            Some(buffer.as_mut_ptr().cast()),
            &mut header,
            DIB_RGB_COLORS,
        );
        let _ = DeleteDC(dc);
        if lines != height {
            return None;
        }
        let has_alpha = buffer.as_chunks::<4>().0.iter().any(|p| p[3] != 0);
        for pixel in buffer.as_chunks_mut::<4>().0 {
            pixel.swap(0, 2);
            if !has_alpha {
                pixel[3] = 255;
            }
            // Shell images are premultiplied; Slint's from_rgba8 expects straight alpha.
            let alpha = u32::from(pixel[3]);
            if alpha > 0 && alpha < 255 {
                for c in &mut pixel[..3] {
                    *c = (u32::from(*c) * 255 / alpha).min(255) as u8;
                }
            }
        }
        Some(Pixels {
            width: width as u32,
            height: height as u32,
            rgba: buffer,
        })
    }
}

//! Background-only opacity. Never change global HWND/content opacity or capture the desktop.
#[cfg(test)]
pub fn opacity(software: bool, configured: bool, composition: bool) -> f32 {
    if !software && configured && composition {
        opacity_percent(software, configured, composition, 92)
    } else {
        1.0
    }
}
pub fn for_platform(software: bool, configured: bool, percent: u8) -> f32 {
    let composition = if !software && configured {
        unsafe {
            windows::Win32::Graphics::Dwm::DwmIsCompositionEnabled()
                .is_ok_and(|enabled| enabled.as_bool())
        }
    } else {
        false
    };
    opacity_percent(software, configured, composition, percent)
}
pub fn opacity_percent(software: bool, configured: bool, composition: bool, percent: u8) -> f32 {
    if !software && configured && composition {
        f32::from(percent.clamp(85, 100)) / 100.
    } else {
        1.0
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unsupported_or_disabled_combinations_are_opaque() {
        assert_eq!(opacity(false, true, true), 0.92);
        for state in [
            (true, true, true),
            (false, true, false),
            (false, false, true),
            (true, false, false),
        ] {
            assert_eq!(opacity(state.0, state.1, state.2), 1.0);
        }
    }
    #[test]
    fn configured_percent_only_affects_supported_backgrounds() {
        for percent in 85..=100 {
            assert_eq!(
                opacity_percent(false, true, true, percent),
                f32::from(percent) / 100.
            );
            assert_eq!(opacity_percent(true, true, true, percent), 1.0);
            assert_eq!(opacity_percent(false, true, false, percent), 1.0);
        }
        assert_eq!(opacity_percent(false, true, true, 0), 0.85);
        assert_eq!(opacity_percent(false, true, true, 255), 1.0);
    }
}

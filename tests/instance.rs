use spotlight_prototype::instance::{IDENTITY, Instance};
fn name() -> String {
    format!(
        "Local\\Spotlight-Isolated-Test-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    )
}
#[test]
fn first_owner_excludes_another_thread_until_shutdown() {
    let n = name();
    assert_ne!(n, IDENTITY);
    let owner = Instance::named(&n).unwrap().unwrap();
    let second = n.clone();
    assert!(
        std::thread::spawn(move || Instance::named(&second).unwrap().is_none())
            .join()
            .unwrap()
    );
    drop(owner);
    assert!(
        std::thread::spawn(move || Instance::named(&n).unwrap().is_some())
            .join()
            .unwrap()
    );
    assert!(!IDENTITY.contains(".exe"));
}
#[test]
fn simultaneous_launch_attempts_have_one_owner() {
    let name = format!("{}-race", name());
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
    let threads: Vec<_> = (0..8)
        .map(|_| {
            let name = name.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                let guard = Instance::named(&name).unwrap();
                let won = guard.is_some();
                barrier.wait(); // winner cannot drop its guard before all try acquiring
                drop(guard);
                won
            })
        })
        .collect();
    assert_eq!(
        threads
            .into_iter()
            .map(|t| usize::from(t.join().unwrap()))
            .sum::<usize>(),
        1
    );
}

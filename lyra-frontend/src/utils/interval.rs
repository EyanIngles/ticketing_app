use dioxus::prelude::*;

/// Increments a tick signal every `ms` milliseconds while the component is mounted.
pub fn use_interval_tick(ms: u32) -> Signal<u32> {
    let mut tick = use_signal(|| 0u32);
    use_hook(move || {
        spawn(async move {
            loop {
                gloo_timers::future::TimeoutFuture::new(ms).await;
                tick += 1;
            }
        });
    });
    tick
}

use dioxus::prelude::*;

/// Initializes a runtime timer that tracks elapsed days since app start
pub fn use_runtime_timer(mut runtime_days: Signal<f64>) {
    use_future(move || async move {
        let start = std::time::Instant::now();
        loop {
            let elapsed_days = start.elapsed().as_secs_f64() / 86_400.0;
            runtime_days.set(elapsed_days);
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    });
}

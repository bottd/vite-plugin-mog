use std::cell::RefCell;

#[derive(Default)]
enum Sink {
    #[default]
    Stderr,
    Capture(Vec<String>),
    Silent,
}

thread_local! {
    static SINK: RefCell<Sink> = const { RefCell::new(Sink::Stderr) };
}

pub fn warn(message: impl Into<String>) {
    let message = message.into();
    SINK.with_borrow_mut(|sink| match sink {
        Sink::Capture(messages) => messages.push(message),
        Sink::Stderr => eprintln!("Warning: {message}"),
        Sink::Silent => {}
    });
}

pub fn capture<T>(run: impl FnOnce() -> T) -> (T, Vec<String>) {
    let _guard = Scope::enter(Sink::Capture(Vec::new()));
    let value = run();
    let messages = SINK.with_borrow_mut(|sink| match sink {
        Sink::Capture(messages) => std::mem::take(messages),
        _ => unreachable!("capture scope restores its sink"),
    });
    (value, messages)
}

pub fn silence<T>(run: impl FnOnce() -> T) -> T {
    let _guard = Scope::enter(Sink::Silent);
    run()
}

struct Scope(Sink);

impl Scope {
    fn enter(next: Sink) -> Self {
        Self(SINK.with(|sink| sink.replace(next)))
    }
}

impl Drop for Scope {
    fn drop(&mut self) {
        SINK.with(|sink| sink.replace(std::mem::take(&mut self.0)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_capture_and_silence_restore_the_outer_sink() {
        let (_, outer) = capture(|| {
            warn("before");
            let (_, inner) = capture(|| warn("inner"));
            assert_eq!(inner, ["inner"]);
            silence(|| warn("discarded"));
            warn("after");
        });
        assert_eq!(outer, ["before", "after"]);
    }

    #[test]
    fn unwinding_restores_the_outer_sink() {
        let (_, outer) = capture(|| {
            let result = std::panic::catch_unwind(|| {
                capture(|| {
                    warn("discarded");
                    panic!("test unwind");
                });
            });
            assert!(result.is_err());
            warn("after panic");
        });
        assert_eq!(outer, ["after panic"]);
    }
}

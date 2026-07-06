/// Wraps an expression transparently unless the `measure-time` feature is enabled,
/// in which case it logs execution duration to the `perf` target.
///
/// Use `measure!(expr)` for quick instrumentation (label derived from
/// `stringify!(expr)`), or `measure!("label", expr)` to provide a stable,
/// concise log label for large blocks/closures.
///
/// Returns the wrapped expression result.
#[macro_export]
macro_rules! measure {
    // Labeled form for block/closure-heavy call sites where stringify! would be noisy.
    ($label:expr, $call:expr $(,)?) => {{
        #[cfg(feature = "measure-time")]
        {
            let start = std::time::Instant::now();
            let result = $call; // evaluated exactly once
            log::info!(
                target: "perf",
                "[measure-time] {} took {:?}",
                $label,
                start.elapsed()
            );
            result
        }

        #[cfg(not(feature = "measure-time"))]
        {
            $call
        }
    }};

    // Wrap any call expression: function call, method call, closure call, etc.
    ($call:expr $(,)?) => {{
        #[cfg(feature = "measure-time")]
        {
            let start = std::time::Instant::now();
            let result = $call; // evaluated exactly once
            log::info!(
                target: "perf",
                "[measure-time] {} took {:?}",
                stringify!($call),
                start.elapsed()
            );
            result
        }

        #[cfg(not(feature = "measure-time"))]
        {
            $call
        }
    }};
}

mod tests {
    use std::thread::sleep;
    use std::time;

    #[allow(dead_code)]
    fn test_function() {
        sleep(time::Duration::from_secs(1));
    }

    #[test]
    fn can_measure_function_timing() {
        measure!(test_function());
    }

    #[test]
    fn can_measure_with_custom_label() {
        let value = measure!("custom-label", {
            sleep(time::Duration::from_millis(1));
            42
        });

        assert_eq!(value, 42);
    }
}

/// Wraps a function transparently unless the measure-time feature
/// flag is utilized -- then it logs the execution time of each function
/// wrapped. This is meant to be used in cases where you either want to collect
/// a lot of performance related data or not based on a compilation feature, not
/// as a runtime-checked methodology like the client has.
///
/// Returns whatever the function that is wrapped does
#[macro_export]
macro_rules! measure {
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
    use core::time;
    use std::thread::sleep;

    #[allow(dead_code)]
    fn test_function() {
        sleep(time::Duration::from_secs(1));
    }

    #[test]
    fn can_measure_function_timing() {
        measure!(test_function());
    }
}

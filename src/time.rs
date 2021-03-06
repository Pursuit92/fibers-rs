// Copyright (c) 2016 DWANGO Co., Ltd. All Rights Reserved.
// See the LICENSE file at the top-level directory of this distribution.


// pub trait TimeoutExt: Sized {
//     fn timeout(self, duration: Duration) -> BoxFuture<Self::Item, Option<Self::Error>>;
// }
// impl <T:Future> TimeoutExt for T {
//     fn timeout(self, duration: Duration) -> BoxFuture<Self::Item, Option<Self::Error>> {
//         panic!()
//     }
// }

//! Time related functionalities.
pub mod timer {
    //! Timer
    use std::time;
    use std::sync::mpsc as std_mpsc;
    use futures::{Future, Poll, Async};

    use internal::io_poll;
    use fiber;

    /// A timer related extension of the `Future` trait.
    pub trait TimerExt: Sized + Future {
        /// Adds the specified timeout to this future.
        fn timeout_after(self, duration: time::Duration) -> TimeoutAfter<Self> {
            TimeoutAfter {
                future: self,
                timeout: timeout(duration),
            }
        }
    }
    impl<T: Future> TimerExt for T {}

    /// A future which will try executing `T` within the specified time duration.
    ///
    /// If the timeout duration passes, it will return `Err(None)`.
    /// If an error occurres before the expiration time, this will result in `Err(Some(T::Error))`.
    pub struct TimeoutAfter<T> {
        future: T,
        timeout: Timeout,
    }
    impl<T: Future> Future for TimeoutAfter<T> {
        type Item = T::Item;
        type Error = Option<T::Error>;
        fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
            if let Async::Ready(value) = self.future.poll().map_err(Some)? {
                Ok(Async::Ready(value))
            } else if let Ok(Async::NotReady) = self.timeout.poll() {
                Ok(Async::NotReady)
            } else {
                Err(None)
            }
        }
    }

    /// A future which will expire at the specified time instant.
    ///
    /// If this object is dropped before expiration, the timer will be cancelled.
    /// Thus, for example, the repetation of setting and canceling of
    /// a timer only consumpts constant memory region.
    #[derive(Debug)]
    pub struct Timeout {
        start: time::Instant,
        duration: time::Duration,
        inner: Option<io_poll::Timeout>,
    }

    /// Makes a future which will expire after `delay_from_now`.
    pub fn timeout(delay_from_now: time::Duration) -> Timeout {
        Timeout {
            start: time::Instant::now(),
            duration: delay_from_now,
            inner: None,
        }
    }
    impl Future for Timeout {
        type Item = ();
        type Error = std_mpsc::RecvError;
        fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
            if let Some(ref mut inner) = self.inner {
                inner.poll()
            } else {
                let elapsed = self.start.elapsed();
                if elapsed >= self.duration {
                    return Ok(Async::Ready(()));
                }
                if let Some(inner) = fiber::with_current_context(|mut c| {
                    let rest = self.duration - elapsed;
                    io_poll::poller::set_timeout(c.poller(), rest)
                }) {
                    self.inner = Some(inner);
                    self.poll()
                } else {
                    Ok(Async::NotReady)
                }
            }
        }
    }

    #[cfg(test)]
    mod test {
        use std::time::Duration;
        use futures::{self, Future, Async};
        use super::*;

        #[test]
        fn it_works() {
            let mut timeout = timeout(Duration::from_secs(0));
            assert_eq!(timeout.poll(), Ok(Async::Ready(())));
        }

        #[test]
        fn timeout_after_works() {
            let mut future = futures::empty::<(), ()>().timeout_after(Duration::from_secs(0));
            assert_eq!(future.poll(), Err(None));

            let mut future = futures::finished::<(), ()>(()).timeout_after(Duration::from_secs(1));
            assert_eq!(future.poll(), Ok(Async::Ready(())));

            let mut future = futures::failed::<(), ()>(()).timeout_after(Duration::from_secs(1));
            assert_eq!(future.poll(), Err(Some(())));
        }
    }
}

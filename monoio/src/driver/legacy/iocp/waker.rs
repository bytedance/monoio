use std::{io, sync::Arc};

use super::{CompletionPort, Event, Poller};

#[derive(Debug)]
pub struct Waker {
    token: mio::Token,
    port: Arc<CompletionPort>,
}

impl Waker {
    pub fn new(poller: &Poller, token: mio::Token) -> io::Result<Waker> {
        Ok(Waker {
            token,
            port: poller.cp.clone(),
        })
    }

    pub fn wake(&self) -> io::Result<()> {
        let mut ev = Event::new(self.token);
        ev.set_readable();

        self.port.post(ev.to_completion_status())
    }
}

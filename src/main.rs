#![feature(async_await)]

use core::{
    pin::Pin,
    task::{Context, Poll},
};

use std::{
    collections::HashMap,
    convert::TryInto,
    ffi::CStr,
    io,
    io::prelude::*,
    io::Error,
    net::{Shutdown, TcpListener, TcpStream},
    os::unix::io::{AsRawFd, RawFd},
    rc::Rc,
    sync::Mutex,
};

use futures::{
    executor::{LocalPool, LocalSpawner},
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    stream::{Stream, TryStreamExt},
    task::{LocalSpawnExt, Waker},
};

/// Look how easy it is to be async :)
fn main() {
    // Set up the listener using the standard library listener
    // Looks easy and follows the same paradigm as blocking.
    let listener = TcpListener::bind("0.0.0.0:7878").unwrap();
    // Now I wrap it up in a server.
    let mut server = Server::new(listener);
    // This will block forever consuming new connections and handling them
    // all asynchronously and on the same thread.
    server.run();
}

/// Wraps the epoll libc interface.
struct Epoll {
    epfd: libc::c_int,
    wakers: Mutex<HashMap<RawFd, Vec<Waker>>>,
}

impl Epoll {
    //Constructor
    pub fn new() -> Epoll {
        let mut ret = Epoll {
            epfd: 0,
            wakers: Mutex::new(HashMap::new()),
        };
        unsafe {
            let r = libc::epoll_create1(0);
            if r == -1 {
                panic!("Epoll create returned -1");
                //There was an error
                //let e = libc::__errno_location()
                //In any case return the error
                //io::Error::new(ErrorKind::Other, "oh no!");
            }
            ret.epfd = r;
        }
        println!("epoll:create epfd:{}", ret.epfd);
        ret
    }

    pub fn ctl_add_rawfd(&self, fd: RawFd, waker: Waker, events: u32) -> Result<(), io::Error> {
        let mut wakers = self.wakers.lock().unwrap();
        let mut event = libc::epoll_event {
            events: events,
            u64: fd.try_into().unwrap(),
        };
        let r = unsafe { libc::epoll_ctl(self.epfd, libc::EPOLL_CTL_ADD, fd, &mut event) };
        if r == -1 {
            let strerror = unsafe {
                let se = libc::strerror(*libc::__errno_location());
                CStr::from_ptr(se).to_string_lossy()
            };
            println!("epoll:ctl_add_rawfd:error: {}", strerror);
            return Err(io::Error::new(io::ErrorKind::Other, strerror));
        }
        if let Some(wakers) = wakers.insert(fd, vec![waker]) {
            for w in wakers {
                w.wake();
            }
        }
        println!("epoll:ctl_add_rawfd fd:{} to epfd:{}", fd, self.epfd);
        Ok(())
    }

    pub fn ctl_mod_rawfd(&self, fd: RawFd, waker: Waker, events: u32) -> Result<(), io::Error> {
        let mut wakers = self.wakers.lock().unwrap();
        let mut event = libc::epoll_event {
            events: events,
            u64: fd.try_into().unwrap(),
        };
        let r = unsafe { libc::epoll_ctl(self.epfd, libc::EPOLL_CTL_MOD, fd, &mut event) };
        if r == -1 {
            let strerror = unsafe {
                let se = libc::strerror(*libc::__errno_location());
                CStr::from_ptr(se).to_string_lossy()
            };
            println!("epoll:ctl_mod_rawfd:error: {}", strerror);
            return Err(io::Error::new(io::ErrorKind::Other, strerror));
        }
        if let Some(wakers) = wakers.insert(fd, vec![waker]) {
            for w in wakers {
                w.wake();
            }
        }
        println!("epoll:ctl_mod_rawfd fd:{} in epfd:{}", fd, self.epfd);
        Ok(())
    }

    pub fn ctl_del_rawfd(&self, fd: RawFd) -> Result<(), io::Error> {
        let mut wakers = self.wakers.lock().unwrap();
        let mut event = libc::epoll_event {
            events: 0,
            u64: fd.try_into().unwrap(),
        };
        let r = unsafe { libc::epoll_ctl(self.epfd, libc::EPOLL_CTL_DEL, fd, &mut event) };
        if r == -1 {
            let strerror = unsafe {
                let se = libc::strerror(*libc::__errno_location());
                CStr::from_ptr(se).to_string_lossy()
            };
            println!("epoll:ctl_del_rawfd:error: {}", strerror);
            return Err(io::Error::new(io::ErrorKind::Other, strerror));
        }
        if let Some(wakers) = wakers.remove(&fd) {
            for w in wakers {
                w.wake();
            }
        }
        println!("epoll:ctl_del_rawfd: fd:{} from epfd:{}", fd, self.epfd);
        Ok(())
    }

    pub fn wait(&self, timeout: i32) -> Result<(), io::Error> {
        let mut events: Vec<libc::epoll_event> = vec![libc::epoll_event { events: 0, u64: 0 }; 10];
        let nfds =
            unsafe { libc::epoll_wait(self.epfd, &mut events[0], events.len() as i32, timeout) };
        if nfds == -1 {
            let strerror = unsafe {
                let se = libc::strerror(*libc::__errno_location());
                CStr::from_ptr(se).to_string_lossy()
            };
            return Err(io::Error::new(io::ErrorKind::Other, strerror));
        }
        //Turn the events array into a vec of file handles
        for event in events.iter().take(nfds as usize) {
            println!("epoll:wait: event:{} for fd:{}", event.events, event.u64);
            {
                let mut wakers = self.wakers.lock().unwrap();
                if let Some(wakers) = wakers.remove(&(event.u64 as RawFd)) {
                    for w in wakers {
                        w.wake();
                    }
                }
            }
        }
        Ok(())
    }
}

impl AsRawFd for Epoll {
    fn as_raw_fd(self: &Self) -> i32 {
        self.epfd
    }
}

impl Drop for Epoll {
    fn drop(&mut self) {
        //Drop this thing
        let r = unsafe { libc::close(self.epfd) };
        if r == -1 {
            let strerror = unsafe {
                let se = libc::strerror(*libc::__errno_location());
                CStr::from_ptr(se).to_string_lossy()
            };
            println!("epoll:drop:error: {}", strerror);
        }
    }
}

/// Consumes and wraps a tcp listener. It will set it in a nonblocking mode. Only allows you to
/// accept incoming connections via listener.incoming().
struct AsyncTcpListener {
    listener: TcpListener,
    epoll: Rc<Epoll>,
    eph: bool,
}

impl AsyncTcpListener {
    pub fn new(listener: TcpListener, epoll: Rc<Epoll>) -> AsyncTcpListener {
        listener.set_nonblocking(true).unwrap();
        AsyncTcpListener {
            listener: listener,
            epoll: epoll,
            eph: false,
        }
    }
    pub fn incoming(&mut self) -> Incoming<'_> {
        Incoming { listener: self }
    }
}

impl Drop for AsyncTcpListener {
    fn drop(&mut self) {
        self.epoll.ctl_del_rawfd(self.listener.as_raw_fd()).unwrap();
    }
}

/// The streaming version of an iterator over accepted tcp connections from the associated
/// listener. Accessed via listener.incoming().
struct Incoming<'a> {
    listener: &'a mut AsyncTcpListener,
}

impl<'a> Stream for Incoming<'a> {
    type Item = Result<AsyncTcpStream, io::Error>;
    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
    ) -> Poll<Option<Self::Item>> {
        let a = self.listener.listener.accept();
        match a {
            Ok((stream, _addr)) => Poll::Ready(Some(Ok(AsyncTcpStream::new(
                stream,
                self.listener.epoll.clone(),
            )))),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                let events = libc::EPOLLET | libc::EPOLLIN | libc::EPOLLONESHOT;
                //If I've already added it to epoll, then I just need to modify now
                if self.listener.eph {
                    self.listener
                        .epoll
                        .ctl_mod_rawfd(
                            self.listener.listener.as_raw_fd(),
                            cx.waker().clone(),
                            events as u32,
                        )
                        .unwrap();
                } else {
                    self.listener
                        .epoll
                        .ctl_add_rawfd(
                            self.listener.listener.as_raw_fd(),
                            cx.waker().clone(),
                            events as u32,
                        )
                        .unwrap();
                    self.listener.eph = true;
                }
                Poll::Pending
            }
            Err(e) => {
                println!("AsyncTcpListener:stream:error {:?}", e);
                Poll::Ready(None)
                //panic!("Error on accept: {}", e);
            }
        }
    }
}

/// Consumes and wraps a tcp stream. It will set it in a nonblocking mode,
/// and provides async versions of read, write, flush, and close from the
/// futures-rs library. At the moment it also takes a reference to an epoll
/// instance that is used under the hood to provide events and wake up the
/// poll methods. Since my impl is all on a single thread, I might be able to
/// use thread local storage to register wakers with an epoll instance.
struct AsyncTcpStream {
    stream: TcpStream,
    epoll: Rc<Epoll>,
    eph: bool,
}

impl AsyncTcpStream {
    pub fn new(stream: TcpStream, epoll: Rc<Epoll>) -> AsyncTcpStream {
        stream.set_nonblocking(true).unwrap();
        AsyncTcpStream {
            stream: stream,
            epoll: epoll,
            eph: false,
        }
    }
}

impl Drop for AsyncTcpStream {
    fn drop(&mut self) {
        self.epoll.ctl_del_rawfd(self.stream.as_raw_fd()).unwrap();
    }
}

impl AsyncRead for AsyncTcpStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &mut [u8],
    ) -> Poll<Result<usize, Error>> {
        let r = self.stream.read(buf);
        match r {
            Ok(i) => Poll::Ready(Ok(i)),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                let events = libc::EPOLLET | libc::EPOLLIN | libc::EPOLLONESHOT;
                if self.eph {
                    self.epoll
                        .ctl_mod_rawfd(self.stream.as_raw_fd(), cx.waker().clone(), events as u32)
                        .unwrap();
                } else {
                    self.epoll
                        .ctl_add_rawfd(self.stream.as_raw_fd(), cx.waker().clone(), events as u32)
                        .unwrap();
                    self.eph = true
                }
                Poll::Pending
            }
            Err(e) => Poll::Ready(Err(e)),
        }
    }
}

impl AsyncWrite for AsyncTcpStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &[u8],
    ) -> Poll<Result<usize, Error>> {
        let r = self.stream.write(buf);
        match r {
            Ok(i) => Poll::Ready(Ok(i)),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                let events = libc::EPOLLET | libc::EPOLLOUT | libc::EPOLLONESHOT;
                if self.eph {
                    self.epoll
                        .ctl_mod_rawfd(self.stream.as_raw_fd(), cx.waker().clone(), events as u32)
                        .unwrap();
                } else {
                    self.epoll
                        .ctl_add_rawfd(self.stream.as_raw_fd(), cx.waker().clone(), events as u32)
                        .unwrap();
                    self.eph = true;
                }
                Poll::Pending
            }
            Err(e) => Poll::Ready(Err(e)),
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), Error>> {
        let r = self.stream.flush();
        match r {
            Ok(()) => Poll::Ready(Ok(())),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                let events = libc::EPOLLET | libc::EPOLLOUT | libc::EPOLLONESHOT;
                if self.eph {
                    self.epoll
                        .ctl_mod_rawfd(self.stream.as_raw_fd(), cx.waker().clone(), events as u32)
                        .unwrap();
                } else {
                    self.epoll
                        .ctl_add_rawfd(self.stream.as_raw_fd(), cx.waker().clone(), events as u32)
                        .unwrap();
                    self.eph = true;
                }
                Poll::Pending
            }
            Err(e) => Poll::Ready(Err(e)),
        }
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), Error>> {
        let r = self.stream.shutdown(Shutdown::Write);
        match r {
            Ok(()) => Poll::Ready(Ok(())),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                let events = libc::EPOLLET | libc::EPOLLOUT | libc::EPOLLONESHOT;
                if self.eph {
                    self.epoll
                        .ctl_mod_rawfd(self.stream.as_raw_fd(), cx.waker().clone(), events as u32)
                        .unwrap();
                } else {
                    self.epoll
                        .ctl_add_rawfd(self.stream.as_raw_fd(), cx.waker().clone(), events as u32)
                        .unwrap();
                    self.eph = true;
                }
                Poll::Pending
            }
            Err(e) => Poll::Ready(Err(e)),
        }
    }
}

/// A Server handles accepting new connections, and handling those connections asynchronously.
/// It uses epoll, and a futures-rs local pool to do so on the same thread by only running the
/// local pool until it is stalled.
struct Server {
    listener: Option<AsyncTcpListener>,
    local_pool: LocalPool,
    epoll: Rc<Epoll>,
}

impl Server {
    /// Creating a new server will consume a tcp listener, turn it async and wrap it with a simple server
    /// that will handle everything asynchronously
    pub fn new(listener: TcpListener) -> Server {
        let local_pool = LocalPool::new();
        let epoll = Rc::new(Epoll::new());
        Server {
            listener: Some(AsyncTcpListener::new(listener, epoll.clone())),
            local_pool: local_pool,
            epoll: epoll.clone(),
        }
    }

    /// The run function will block the thread while it accepts and handles incoming connections to
    /// the associated handler. It uses a futures-rs executer to asynchronously handle everything on
    /// the current thread.
    pub fn run(self: &mut Self) {
        //How do I get my stream to run on this executer, spawning for each new iteration?
        self.local_pool
            .spawner()
            .spawn_local(accept_async(
                self.listener.take().unwrap(),
                self.local_pool.spawner(),
            ))
            .unwrap();
        loop {
            //Run the localPool as far as you can without blocking...
            self.local_pool.run_until_stalled();
            //I'm now noticing that anything that was spawned didn't get grabbed ^ that round
            //Let's run through it again to get all the newly spawned futures to call poll
            self.local_pool.run_until_stalled();
            //Ok now everything is just waiting on io...
            //Let epoll take over, epoll will block on all the fd it's gathered
            //when finished it will notify all the relevant wakers that had events
            self.epoll.wait(-1).unwrap();
            //After epoll is done waiting, rerun the loop
            println!("loop: Another round");
        }
    }
}

/// This future won't resolve... well maybe it should if someone calls close on the listener
/// It's job is to loop over an async iterator (stream) of incoming connections.
/// It will accept the connection, and spawn a new future to handle that connection.
async fn accept_async(mut listener: AsyncTcpListener, mut spawner: LocalSpawner) {
    let mut incoming = listener.incoming();
    loop {
        let next = incoming.try_next().await;
        if let Ok(r) = next {
            if let Some(stream) = r {
                spawner
                    .spawn_local(handle_connection_async(stream))
                    .unwrap();
            } else {
                println!("accept_async:error");
            }
        } else {
            println!("accept_async:none");
        }
    }
}

/// This function takes control of an asynctcpstream
/// It will async read, write, flush, and eventually close the connection
/// Eventually this function might handle the http protocol
/// Read the incoming request, validate headers, create a request obj with a async reader attached
/// to read the body. It could then find an associated handler registered to the server by path
/// match and call it to get a response back.
async fn handle_connection_async(mut stream: AsyncTcpStream) {
    println!("handle_connection_async: Handling:started...");
    //stream.shutdown(Shutdown::Both);
    let mut buffer = [0u8; 2048];
    stream.read(&mut buffer).await.unwrap();
    println!();
    println!("Request: {}", String::from_utf8_lossy(&buffer[..]));
    println!();
    let response = "HTTP/1.1 200 OK\r\n\r\n";

    stream.write_all(response.as_bytes()).await.unwrap();
    stream.flush().await.unwrap();
    stream.close().await.unwrap();
    println!("handle_connection_async: Handling:finished");
}

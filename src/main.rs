#![feature(async_await)]

use core::{
    pin::Pin,
    task::{Context, Poll},
};

use std::{
    collections::HashMap,
    convert::TryInto,
    ffi::{CStr, CString},
    io,
    io::prelude::*,
    io::Error,
    net::{Shutdown, SocketAddr, TcpListener, TcpStream},
    os::unix::io::{AsRawFd, IntoRawFd, RawFd},
    rc::Rc,
    sync::Mutex,
};

use futures::{
    executor::{LocalPool, LocalSpawner},
    future::{empty, ready, BoxFuture, FutureObj, LocalFutureObj},
    stream::{self, Stream, StreamExt, TryStream, TryStreamExt},
    task::{LocalSpawn, LocalSpawnExt, Spawn, SpawnExt, Waker},
    io::{AsyncRead, AsyncWrite, AsyncReadExt, AsyncWriteExt},
};

fn main() {
    let listener = TcpListener::bind("0.0.0.0:7878").unwrap();
    let mut server = Server::new(listener);
    server.run();
}

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
    fn drop(&mut self){
        self.epoll.ctl_del_rawfd(self.listener.as_raw_fd());
    }
}

struct Incoming<'a> {
    listener: &'a mut AsyncTcpListener,
}

impl<'a> TryStream for Incoming<'a> {
    type Ok = AsyncTcpStream;
    type Error = io::Error;
    fn try_poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Result<Self::Ok,Self::Error>>> {
        let a = self.listener.listener.accept();
        match a {
            Ok((stream, _addr)) => Poll::Ready(Some(Ok(AsyncTcpStream::new(
                stream,
                self.listener.epoll.clone(),
            )))),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                let events = libc::EPOLLET | libc::EPOLLIN | libc::EPOLLONESHOT;
                //If I've already added it to epoll, then I just need to modify now
                if self.listener.eph{
                    self.listener.epoll.ctl_mod_rawfd(
                        self.listener.listener.as_raw_fd(),
                        cx.waker().clone(),
                        events as u32,
                    );
                }else{
                    self.listener.epoll.ctl_add_rawfd(
                        self.listener.listener.as_raw_fd(),
                        cx.waker().clone(),
                        events as u32,
                    );
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
    fn drop(&mut self){
        self.epoll.ctl_del_rawfd(self.stream.as_raw_fd());
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
                if self.eph{
                    self.epoll.ctl_mod_rawfd(
                        self.stream.as_raw_fd(),
                        cx.waker().clone(),
                        events as u32,
                        );
                }else{
                    self.epoll.ctl_add_rawfd(
                        self.stream.as_raw_fd(),
                        cx.waker().clone(),
                        events as u32,
                        );
                    self.eph = true
                }
                Poll::Pending
            },
            Err(e) => {
                Poll::Ready(Err(e))
            }
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
                if self.eph{
                    self.epoll.ctl_mod_rawfd(
                        self.stream.as_raw_fd(),
                        cx.waker().clone(),
                        events as u32);
                }else{
                    self.epoll.ctl_add_rawfd(
                        self.stream.as_raw_fd(),
                        cx.waker().clone(),
                        events as u32);
                    self.eph = true;
                }
                Poll::Pending
            },
            Err(e) =>{
                Poll::Ready(Err(e))
            }
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), Error>> {
        let r = self.stream.flush();
        match r {
            Ok(()) => Poll::Ready(Ok(())),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                let events = libc::EPOLLET | libc::EPOLLOUT | libc::EPOLLONESHOT;
                if self.eph{
                    self.epoll.ctl_mod_rawfd(
                        self.stream.as_raw_fd(),
                        cx.waker().clone(),
                        events as u32);
                }else{
                    self.epoll.ctl_add_rawfd(
                        self.stream.as_raw_fd(),
                        cx.waker().clone(),
                        events as u32);
                    self.eph = true;
                }
                Poll::Pending
            },
            Err(e) =>{
                Poll::Ready(Err(e))
            }
        }
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), Error>> {
        let r = self.stream.shutdown(Shutdown::Write);
        match r {
            Ok(()) => Poll::Ready(Ok(())),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                let events = libc::EPOLLET | libc::EPOLLOUT | libc::EPOLLONESHOT;
                if self.eph{
                    self.epoll.ctl_mod_rawfd(
                        self.stream.as_raw_fd(),
                        cx.waker().clone(),
                        events as u32);
                }else{
                    self.epoll.ctl_add_rawfd(
                        self.stream.as_raw_fd(),
                        cx.waker().clone(),
                        events as u32);
                    self.eph = true;
                }
                Poll::Pending
            },
            Err(e) =>{
                Poll::Ready(Err(e))
            }
        }
    }
}

struct Server {
    listener: Option<AsyncTcpListener>,
    localPool: LocalPool,
    epoll: Rc<Epoll>,
}

impl Server {
    pub fn new(listener: TcpListener) -> Server {
        let localPool = LocalPool::new();
        let epoll = Rc::new(Epoll::new());
        Server {
            listener: Some(AsyncTcpListener::new(listener, epoll.clone())),
            localPool: localPool,
            epoll: epoll.clone(),
        }
    }

    pub fn run(self: &mut Self) {
        //How do I get my stream to run on this executer, spawning for each new iteration?
        self.localPool.spawner()
            .spawn_local(accept_async(self.listener.take().unwrap(), self.localPool.spawner()));
        loop {
            //Run the localPool as far as you can without blocking...
            self.localPool.run_until_stalled();
            //I'm now noticing that anything that was spawned didn't get grabbed ^ that round
            self.localPool.run_until_stalled();
            //Now let epoll take over, epoll will block on all the fd it's gathered
            self.epoll.wait(-1);
            //After epoll is done waiting, rerun the loop
            //It has notified all the relevant wakers that had events
            println!("loop: Another round");
        }
    }
}

async fn accept_async(mut listener: AsyncTcpListener, mut spawner : LocalSpawner) {
    let mut incoming = listener.incoming();
    loop{
        let next = incoming.try_next().await;
        if let Ok(r) = next{
            if let Some(stream) = r{
                let sc = spawner.clone();
                spawner.spawn_local(handle_connection_async(stream, sc));
            }else{
                println!("accept_async:error");
            }
        }else{
            println!("accept_async:none");
        }
    }
}

async fn handle_connection_async(mut stream: AsyncTcpStream, spawner: LocalSpawner) {
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

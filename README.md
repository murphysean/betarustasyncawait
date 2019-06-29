Rust async await
===

An attempt to learn and expirement with the whole async stack.

Introduction
---

Learning rust, and coming from go, want to see where async await is, my primary reason for holding off on adopting rust fully.
I've attended a few user groups and read the book, wanted to dig in and get the feel.

This includes:

* The actual system calls epoll
* Use streams/iterators and futures from futures-rs package
* A higher level server that consumes an existing listener and turns it async
* Attempt to get the executer and the epoll to run on the same thread, everything on the same thread
* async functions that handle connections
* .await on things like read, write, flush, close

References
---

Here are some websites I used:

* https://doc.rust-lang.org/stable/book/ch20-01-single-threaded.html
* https://rust-lang-nursery.github.io/futures-api-docs/0.3.0-alpha.16/futures/index.html
* http://man7.org/linux/man-pages/man7/epoll.7.html
* https://docs.rs/epoll/4.1.0/epoll/struct.Events.html
* https://docs.rs/libc/0.2.58/libc/index.html

Example Output
---

### Rust Server

	Created epoll fd:5
	AsyncTcpListener:Stream:Accept would block: Resource temporarily unavailable (os error 11)
	ctl_add_rawfd
	Added fd:4 to epollfd:5
	Wait: 1 for 4
	loop: Another round
	AsyncTcpListener:Stream:Accept would block: Resource temporarily unavailable (os error 11)
	ctl_mod_rawfd
	Added fd:4 to epollfd:5
	Handling:started...
	AsyncStream:Read:Read would block: Resource temporarily unavailable (os error 11)
	ctl_add_rawfd
	Added fd:6 to epollfd:5
	Wait: 1 for 6
	loop: Another round
	Request: GET / HTTP/1.1
	Host: localhost:7878
	User-Agent: curl/7.64.0
	Accept: */*


	Handling:finished
	Dropping stream
	ctl_del_rawfd
	ctl_del_rawfd: fd:6 from epollfd:5

### Connection

	smurphy-ml-6:~ seanmurphy$ curl -v localhost:7878
	* Expire in 0 ms for 6 (transfer 0x7fd69700a200)
	* Expire in 1 ms for 1 (transfer 0x7fd69700a200)
	* Expire in 0 ms for 1 (transfer 0x7fd69700a200)
	* Expire in 1 ms for 1 (transfer 0x7fd69700a200)
	* Expire in 0 ms for 1 (transfer 0x7fd69700a200)
	* Expire in 0 ms for 1 (transfer 0x7fd69700a200)
	* Expire in 1 ms for 1 (transfer 0x7fd69700a200)
	* Expire in 0 ms for 1 (transfer 0x7fd69700a200)
	* Expire in 0 ms for 1 (transfer 0x7fd69700a200)
	* Expire in 1 ms for 1 (transfer 0x7fd69700a200)
	* Expire in 0 ms for 1 (transfer 0x7fd69700a200)
	* Expire in 0 ms for 1 (transfer 0x7fd69700a200)
	* Expire in 1 ms for 1 (transfer 0x7fd69700a200)
	* Expire in 0 ms for 1 (transfer 0x7fd69700a200)
	* Expire in 0 ms for 1 (transfer 0x7fd69700a200)
	* Expire in 1 ms for 1 (transfer 0x7fd69700a200)
	* Expire in 0 ms for 1 (transfer 0x7fd69700a200)
	* Expire in 0 ms for 1 (transfer 0x7fd69700a200)
	* Expire in 1 ms for 1 (transfer 0x7fd69700a200)
	* Expire in 0 ms for 1 (transfer 0x7fd69700a200)
	* Expire in 0 ms for 1 (transfer 0x7fd69700a200)
	* Expire in 1 ms for 1 (transfer 0x7fd69700a200)
	* Expire in 0 ms for 1 (transfer 0x7fd69700a200)
	* Expire in 0 ms for 1 (transfer 0x7fd69700a200)
	* Expire in 1 ms for 1 (transfer 0x7fd69700a200)
	* Expire in 0 ms for 1 (transfer 0x7fd69700a200)
	* Expire in 0 ms for 1 (transfer 0x7fd69700a200)
	* Expire in 1 ms for 1 (transfer 0x7fd69700a200)
	* Expire in 0 ms for 1 (transfer 0x7fd69700a200)
	* Expire in 0 ms for 1 (transfer 0x7fd69700a200)
	* Expire in 1 ms for 1 (transfer 0x7fd69700a200)
	* Expire in 0 ms for 1 (transfer 0x7fd69700a200)
	* Expire in 0 ms for 1 (transfer 0x7fd69700a200)
	* Expire in 1 ms for 1 (transfer 0x7fd69700a200)
	* Expire in 0 ms for 1 (transfer 0x7fd69700a200)
	* Expire in 0 ms for 1 (transfer 0x7fd69700a200)
	* Expire in 1 ms for 1 (transfer 0x7fd69700a200)
	* Expire in 0 ms for 1 (transfer 0x7fd69700a200)
	* Expire in 0 ms for 1 (transfer 0x7fd69700a200)
	* Expire in 1 ms for 1 (transfer 0x7fd69700a200)
	* Expire in 0 ms for 1 (transfer 0x7fd69700a200)
	* Expire in 0 ms for 1 (transfer 0x7fd69700a200)
	* Expire in 0 ms for 1 (transfer 0x7fd69700a200)
	*   Trying ::1...
	* TCP_NODELAY set
	* Expire in 149999 ms for 3 (transfer 0x7fd69700a200)
	* Expire in 200 ms for 4 (transfer 0x7fd69700a200)
	* Connected to localhost (::1) port 7878 (#0)
	> GET / HTTP/1.1
	> Host: localhost:7878
	> User-Agent: curl/7.64.0
	> Accept: */*
	>
	< HTTP/1.1 200 OK
	* no chunk, no close, no size. Assume close to signal end
	<
	* Closing connection 0

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

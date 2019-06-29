Rust async await
===

An attempt to learn and experiment with the whole async stack.

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

Improvements
---

### Epoll

In epoll I should probably hold on to wakers split out by read/write ops.
Right now it only holds on to a set of wakers for a single fd.

Do you allow multiple wakers by fd-op?
It seems like you'd really only want one waker at a time.

I initially designed epoll to be able to work on it's own thread.
It would just run the wait function over and over.
The other threads would come in and add fd-ops.
This is why it uses a mutex, to control access to the hashmap.

On a single thread it's not as big a deal, and now that I have it running on a single thread, I can probably remove the mutex.

But this does bring up an interesting point about how to scale this.
Obviously you'd want to eventually use the ThreadPool executer (one per core) instead of the LocalPool.

Now it gets tricky, you could just have a separate thread running your epoll wait.
The other threads would register their wakers with this thread, and it would in turn wake them up.
But what if it's in a wait for a set of files that don't have any activity.
And another file gets added to the interest list.
Those files might have high activity and need the events to come back sooner than the initial set will.
Now you need to interrupt the wait. I guess you could use pwait or something, and get into sending a signal to that thread.
You'd do this every time a new one comes in.
I think this might lend itself better to a work stealing type approach, 
where threads that are about to park can check with threads that are busy and pull some work off of them. 
Because they share the same foreign thread to manage epoll access this would work much better.

What if you give each executer it's own epoll instance?
It could work even better if you have each thread also doing it's own accept.
You can have multiple listeners registered on an incoming port in linux AFAIK.

### Futures-rs

Another thing that seems like it'd fit nicely into the futures-rs package is a built in timeout future.
Kind of like the sync primitives they've got there, it'd be nice to be able to easily play around with some timeouts.
I think this would work really nicely for people getting familiar with futures.

Much like epoll, timeouts will require a syscall, sleep in this case.

### Better integration into std

So how do you integrate all this into standard?
At some point it seems like we've got to have the read/write implementations offered on the standard files.

This could be done in futures-rs, backed by mio or something.
But it does require picking an implementation, like do you have the file register itself on a thread local epoll?
Do you construct a new file object and give it an epoll instance to register with.
This is how I did it.

It'd be really nice to just have standard support for at least network and sleeping.
I'd even be ok if this just came from futures-rs.

It feels like std should offer some async primitives at least around files, sockets, etc.

Thoughts
---

Where should the waiting occur?
So one thing I noticed is that I'm encouraged to offload the waiting.
My inclination is that the executer should do the waiting instead of parking the thread.
This is why I don't let the executer park the thread by using some of it's methods that don't block.
Then I pull in the epoll and let it wait.
It seems like I'm being pushed into a design where I start up new threads that will do the waiting, 
and then unpark the executer thread via x-thread comms.

The spawner doesn't immediately poll things added.
I guess this makes sense, but it was kind of weird having to run a non-blocking pass to accept new connections,
and then immediately have to run another pass to get those futures to a block for read.
It'd be nice if the function would account for things that were added in the last pass before returning...


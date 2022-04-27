# tsyncp

### Synchronization primitives over TCP for message passing.

Major rust libraries such as [std] and [tokio] provide great synchronization primitives to be
used for message-passing between threads and tasks. However, there are not many libraries
that provide similar APIs that can be used over the network.

**Tsyncp** tries to fill the gap by providing the similar APIs (mpsc, broadcast, barrier, etc) over TCP. If you
have a project where it only has a few services running, and they need to pass some data to each other;
instead of setting up a message-broker service, you can use tsyncp to easily pass data between them.

**Tsyncp** also allows customizing different Serialization/Deserialization methods to encode/decode
data; currently, supported schemes straight from the library are _Json_, _Protobuf_, ~_Bincode_~, ~_Speedy_~, and ~_Rkyv_~; however,
users can very easily implement their own [EncodeMethod] and [DecodeMethod].

## Provided APIs

Currently, **tsyncp** provides 6 types of channels:

-   **mpsc**: Multi-producer/single-consumer channel.
-   **broadcast**: Sigle-producer/multi-consumer channel.
-   **barrier**: Ensures multiple waiters wait for the barrier to release.
-   **spsc**: Single-producer/single-consumer channel.
-   **channel**: Generic single-connection channel for sending/receiving data.
    Can split into _Sender_ and _Receiver_ pair.
-   **multi_channel**: Generic multi-connection channel for sending/receiving data.
    Can split into _Sender_ and _Receiver_ pair.

The [API documentation](https://docs.rs/tsyncp/) has a very detailed guide on how to use the primitives. So please check them out!

**_Note_**: Tsyncp is built on [tokio]; and thus may not be compatible with other async runtimes like async-std.

**_Note_**: If you're worried about the quality of implementation and skills of the author, you shouldn't worry! (too much...)
This library is a relatively thin layer on top of [tokio::net::TcpStream] for TCP,
[tokio_util::codec::Framed] for byte stream framing, and [serde], [serde_json], [Prost], etc. for serializing byte data!
You can think of this library as a salad bowl made from the best ingredients in town and a light touch of homemade sauces.

That being said, I did add a couple of my own sauces into it. The two flavours that I added are:

-   Very simple implementation of [VariedLengthDelimitedCodec](https://docs.rs/tsyncp/latest/tsyncp/util/frame_codec/struct.VariedLengthDelimitedCodec.html),
    which encodes/decodes varying header length depending on the size of the byte data.
-   Minimal implementation of TCP stream pool [StreamPool](https://docs.rs/tsyncp/latest/tsyncp/util/stream_pool/struct.StreamPool.html),
    which allows pushing and popping streams from the pool, streaming data from the pool, and broadcasting data into the pool.

You can take a look at the implementations of these, which both are pretty simple.
And I tried my best not to put too much of my personal opinions into it!

But still, since this library is still a baby, if you find any concerns in the code, please contact me at **jack.y.l.dev@gmail.com**!

_Warning: Tsyncp is not a message-broker nor it tries to be;
it's just a message-passing library for simple and convenient use cases._

_Warning: Tsyncp is still WIP! It's usable, but still needs some encode/decode features implemented, extensive testing, documentations, and examples._

[std]: https://doc.rust-lang.org/stable/std/
[tokio]: https://docs.rs/tokio/latest/tokio/index.html
[encodemethod]: https://docs.rs/tsyncp/latest/tsyncp/util/codec/trait.EncodeMethod.html
[decodemethod]: https://docs.rs/tsyncp/latest/tsyncp/util/codec/trait.DecodeMethod.html
[tokio::net::tcpstream]: https://docs.rs/tokio/latest/tokio/net/struct.TcpStream.html
[tokio_util::codec::framed]: https://docs.rs/tokio-util/latest/tokio_util/codec/struct.Framed.html
[serde]: https://crates.io/crates/serde
[serde_json]: https://crates.io/crates/serde_json
[prost]: https://crates.io/crates/prost

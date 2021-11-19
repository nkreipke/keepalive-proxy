# A very basic HTTP proxy that "keep-alives" all connections.

**Why would you need this?** Some applications might want to download a lot of small "chunks" of data. They might reconnect after every chunk. Due to the inner workings of TCP, namely the TCP Slow Start algorithm, achieving high throughputs might be impossible in that case. The HTTP `Connection: Keep-Alive` header remediates this, however some software does not utilize it.

**How does the proxy help?** Two assumptions:

- TCP Slow Start can reach high congestion window sizes faster over the loopback device than over the internet, as the RTT is much shorter.
- The initial congestion and receive windows can be adjusted for the loopback device, potentially reducing the impact of TCP Slow Start.

The proxy uses a connection pool to keep outgoing connections open for a few seconds after the client has already disconnected. While clients might establish a new connection for every chunk, the proxy will use a single connection for the whole download process.

**What are the limitations?** HTTPS connections are supported, however they are not connection pooled as we cannot modify the HTTP headers. This might be possible by terminating TLS at the proxy and using a self-signed certificate, however this is not implemented. This proxy is mainly intended for video streaming, which is often plain HTTP.

**How does it perform?** On a 500 MBit/s downlink, I have observed speedups of around 20% with just the proxy alone. By additionally tuning the initial congestion and receive window sizes for the loopback device, the mean throughput can be more than doubled.

```bash
ip route change 127.0.0.1 dev lo table local initcwnd 96 initrwnd 96
```

*Note: I have determined my "optimal" window size of 96 by experimentation, however it probably is not valid for all possible use cases. For best results I recommend you do your own testing.*

**An actual use case and performance comparison:**

[yt-dlp](https://github.com/yt-dlp/yt-dlp) with a video streaming service that shall remain unnamed, with chunks being served by AWS CloudFront: Total download size is around 1.3 GB, each chunk is approximately 2 MB. The data is downloaded to a ramdisk.

```bash
$ time yt-dlp "redacted" --no-progress
________________________________________________________
Executed in  112,75 secs    fish           external
   usr time   12,27 secs    0,00 micros   12,27 secs
   sys time    5,74 secs  425,00 micros    5,74 secs
```

```bash
$ time yt-dlp "redacted" --no-progress --proxy http://127.0.0.1:9250
________________________________________________________
Executed in   44,75 secs    fish           external
   usr time    8,74 secs  488,00 micros    8,74 secs
   sys time    3,81 secs    0,00 micros    3,81 secs
```

*Note: This does not work with YouTube as they use HTTPS on their streaming servers. I might add HTTPS support someday, but today is not the day.*

**Usage:** Get [Rust](https://www.rust-lang.org/). Execute `cargo run --release`. The proxy binds to port 9250.
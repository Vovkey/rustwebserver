RWS

This is my very own rust web server built from scratch.

It basically let's you host a static website, so html, css and JS.

For testing purposes, it has the /sleep route, which has a 5s delay. The /stats route for a statistics overview. The /api/metrics route, which outputs a JSON of the statistics. The /quit rout to shutdown the server. And a test-messaging form.

To test it out, download it and run it with 'cargo run' and test out the different routes. It's running on 127.0.0.1:7878 by default.

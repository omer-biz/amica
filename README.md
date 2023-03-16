# Amica - A Humble Proxy

This is a simple proxy, which gave me the excuse of embedding `lua` in a `Rust`.
This, by no means, is production ready, and it is under heavy development.

## Introduction

The simple idea behind the proxy is to kind of act as a middle man, which can
inspect and change the `Reuest` from the `client` and the `Response` from the
`server`.

### Run

```bash
git clone "https://github.com/omer-biz/amica.git"
cd amica
cargo run
```

### Options

```bash
cargo run -- --help

Usage: amica [OPTIONS]

Options:
  -f, --filter-script <lua script>  Optinal lua script to run on the the intermediate request and response
  -a, --address <ip:port>           Address to bind to
  -h, --help                        Print help
```

### Lua API

The `lua` file must contain two functions with exactly the following signiture

```lua
function on_http_request(req)
  -- getters
  req:uri() 
  req:method() 
  req:body() 
  req:headers() -- request headers as lua tables.

  -- setters
  req:set_uri("http://duckduckgo.com") -- https doesn't work.
  req:set_method("POST")

  -- updates the header value`Content-Length` automatically.
  req:set_body("<h1>Hello from Amica</h1>" .. req:body()) 
  req:set_header("Host", "duckduckgo.com")

  return req
end

function on_http_response(res)
  -- getters
  res:body() -- string
  res:headers() -- table
  res:status() -- number

  -- setters
  res:set_status(500)
  -- updates the header value`Content-Length` automatically.
  res:set_body("<h1>Hello from Amica</h1>" .. req:body()) 
  res:set_header("location", "duckduckgo.com")

  return res
end
```

The method defined on `req` and `res` are provided by `amica` and can be
used to inspect and change the `Request` from the client with `req`, and `Response`
from the server with `res`.

- `on_http_request` is called on the `client`'s `Request`, and
- 'on_http_response' is called on the `server`'s `Response`.

## Contributing

Any help is appreciated, just submit a PR or open an issue.

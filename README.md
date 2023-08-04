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
  -p, --pool-number <pool number>   Number of Proxy pools to spwan. By default it's 1
  -v, --verbose                     Verbosity. if turned on shows the request and response as they are happening for `http` requests
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

## Example Usage

Assume you want every request that is targeted at `google.com` to be redirected to `duckduckgo.com`.
To do that first we create a script, let's call it `no_google.lua`, in the current directory with
the following content.
```lua
function on_http_request(req)
  -- TODO: regex
  if req:uri() == "http://google.com/" then
    req:set_uri("http://duckduckgo.com")
  end

  return req
end
```

Second run the app like this.

```sh
cargo run -- -v --filter-script no_google.lua
Listening on 127.0.0.1:9001
```

Finally direct your clients to this address for example with curl.

```sh
curl -vv google.com --proxy 127.0.0.1:9001
...
> location: https://duckduckgo.com/
...
```

you can see the response is trying to redirect us to `duckduckgo.com` as well as making us use `https`.

## Contributing

Any help is appreciated, just submit a PR or open an issue.

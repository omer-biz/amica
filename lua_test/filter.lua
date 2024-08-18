function on_http_request(req)
  -- if req:uri() == "http://google.com/" then
  -- req:set_uri("http://duckduckgo.com")
  req:set_body("Hello")
  -- end

  return req
end

function on_http_response(res)
  -- res:set_header("location", "https://www.duckduckgo.com")
  res:set_body("hi");

  return res
end

-- -- TODO: enable filtering https requests like this
-- function on_https_request(_)
--   -- uri
--   -- headers
-- end

use chrono::Utc;
use failure::Error;
use http::{Method, Response, Request, Uri, StatusCode};
use http::header::{CONTENT_LENGTH, TRANSFER_ENCODING, SERVER, CONNECTION, DATE, CONTENT_TYPE, HeaderValue};
use log::debug;
use regex::bytes::Regex;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub type Body = Option<String>;

pub async fn http_send_response(mut resp: Response<Body>, mut tx: impl AsyncWriteExt + Unpin) -> Result<(), Error> {

    if !resp.headers().contains_key(CONTENT_LENGTH) && resp.body().is_some() {
        let content_size = resp.body().as_ref().unwrap().len();
        resp.headers_mut().insert(CONTENT_LENGTH, HeaderValue::from_str(&content_size.to_string())?);
    }
    resp.headers_mut().insert(SERVER, HeaderValue::from_static("rp2p"));
    resp.headers_mut().insert(CONNECTION, HeaderValue::from_static("close"));
    resp.headers_mut().insert(DATE, HeaderValue::from_str(&Utc::now().format("%a, %d %m %Y %H:%M:%S GMT").to_string())?);

    let line = format!("HTTP/1.1 {} {}\r\n", resp.status().as_str(), resp.status().canonical_reason().unwrap());
    debug!("Send HTTP header: {:?}", &line);
    tx.write_all(line.as_bytes()).await?;

    for (name, value) in resp.headers() {
        let line = format!("{}: {}\r\n", name.as_str().capitalize(), value.to_str()?);
        debug!("Send header: {:?}", &line);
        tx.write_all(line.as_bytes()).await?;
    }

    let line = "\r\n";
    debug!("Send header terminator");
    tx.write_all(line.as_bytes()).await?;

    if let &Some(ref body) = resp.body() {
        debug!("Send body: {:?}", &body);
        tx.write_all(body.as_bytes()).await?;
    }

    Ok(())
}

pub async fn http_receive_request(mut rx: impl AsyncReadExt + Unpin) -> Result<Request<Body>, Error> {
    let mut request_builder = Request::builder();

    let mut request_data = vec![];
    let mut content_length = None;
    let mut parsed_length = None;
    let mut transfer_encoding = None;

    let mut buf = [0u8; 512];
    while let Ok(bytes_sz) = rx.read(&mut buf).await {
        if bytes_sz == 0 {
            break;
        }

        request_data.extend_from_slice(&buf[0..bytes_sz]);

        let mut headers = [httparse::EMPTY_HEADER; 32];
        let mut request_parser = httparse::Request::new(&mut headers);

        if let httparse::Status::Complete(sz) = request_parser.parse(&request_data[..])? {
            parsed_length = Some(sz);

            // content length
            content_length = request_parser.headers
                .iter()
                .find(|header| {
                    let content_length = CONTENT_LENGTH;
                    header.name.to_lowercase() == content_length.as_str()
                })
                .map(|header| std::str::from_utf8(header.value).unwrap().to_string().parse::<usize>().unwrap());
            // transfer encoding
            transfer_encoding = request_parser.headers
                .iter()
                .find(|header| {
                    let transfer_encoding = TRANSFER_ENCODING;
                    header.name.to_lowercase() == transfer_encoding.as_str()
                })
                .map(|header| std::str::from_utf8(header.value).unwrap().to_string());

            // method
            if let Some(m) = request_parser.method {
                request_builder.method(Method::from_bytes(m.as_bytes())?);
            }
            // path
            if let Some(p) = request_parser.path {
                request_builder.uri(p.parse::<Uri>()?);
            }
            // headers
            for header in request_parser.headers.iter() {
                request_builder.header(header.name, std::str::from_utf8(header.value)?);
            }

            break;
        }
    }

    let mut body = None;

    if let Some(content_length) = content_length {
        let remaining_content_length = parsed_length.unwrap() + content_length - request_data.len();

        debug!("Received data length: {}, content length: {}, parsed length: {:?}, remaining: {}", request_data.len(), content_length, parsed_length, remaining_content_length);

        let mut buf = vec![0u8; remaining_content_length];
        rx.read_exact(&mut buf).await?;
        request_data.extend_from_slice(&buf);

        body = Some(std::str::from_utf8(&request_data[(request_data.len() - content_length)..request_data.len()])?.to_string());
        debug!("Received body: {:?}", &body);
    } else if let Some(transfer_encoding) = transfer_encoding {

        if "chunked".eq_ignore_ascii_case(&transfer_encoding) {
            debug!("Received data length: {}, parsed length: {:?}", request_data.len(), parsed_length);

            // body contents
            let mut body_chunked = vec![];

            let re = Regex::new(r"[0-9a-fA-F]+\r\n")?;
            let mut request_data_drain_idx = parsed_length.unwrap();
            loop {

                let chunk_data = &request_data[request_data_drain_idx..request_data.len()];

                let mut require_more_data = false;
                match re.find(&chunk_data) {
                    Some(chunk_len_match) => {
                        let chunk_len_start = chunk_len_match.start();
                        let chunk_len_end = chunk_len_match.end() - 2; // avoid trailing "\r\n"
                        let chunk_len = usize::from_str_radix(std::str::from_utf8(&chunk_data[chunk_len_start..chunk_len_end])?, 16)?;
                        if chunk_len == 0 {
                            break;
                        }

                        let chunk_start = chunk_len_match.end(); // jump over "\r\n"
                        let chunk_end = chunk_start + chunk_len;

                        if chunk_end < chunk_data.len() {
                            let chunk_data = &chunk_data[chunk_start..chunk_end];
                            body_chunked.extend_from_slice(chunk_data);
                            request_data_drain_idx += chunk_data.len();
                        } else {
                            require_more_data = true;
                        }
                    },
                    None => {
                        require_more_data = true;
                    }
                }

                if require_more_data {
                    let bytes_sz = rx.read(&mut buf).await?;
                    request_data.extend_from_slice(&buf[0..bytes_sz]);
                }

            }

            body = Some(std::str::from_utf8(&body_chunked)?.to_string());
            debug!("Received body: {:?}", &body);
        }
    }

    Ok(request_builder.body(body)?)
}


/// This is convenience function to send http OK json response.
pub async fn http_send_response_ok_json<T: AsyncWriteExt + Unpin + 'static>(msg: &str, tx: T) -> Result<(), Error> {
    let mut resp = Response::new(None);
    *resp.status_mut() = StatusCode::OK;
    resp.headers_mut().insert(CONTENT_TYPE, HeaderValue::from_str("application/json").unwrap());
    *resp.body_mut() = Some(String::from(msg));

    http_send_response(resp, tx).await
}

/// This is convenience function to send http OK text/html response.
pub async fn http_send_response_ok_text<T: AsyncWriteExt + Unpin + 'static>(msg: &str, tx: T) -> Result<(), Error> {
    let mut resp = Response::new(None);
    *resp.status_mut() = StatusCode::OK;
    resp.headers_mut().insert(CONTENT_TYPE, HeaderValue::from_str("text/html").unwrap());
    *resp.body_mut() = Some(String::from(msg));

    http_send_response(resp, tx).await
}

pub trait Capitalize {
    fn capitalize(self) -> String;
}

impl Capitalize for &str {
    fn capitalize(self) -> String {
        let mut c = self.chars();
        match c.next() {
            None => String::new(),
            Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        }
    }
}
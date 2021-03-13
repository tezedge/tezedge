const COMPRESSION_LEVEL : i32 = 2;
fn zstd_compress<B: AsRef<&[u8]>>(input: B, output: &mut Vec<u8>) -> std::io::Result<()>{
    zstd::stream::copy_encode(input, output, COMPRESSION_LEVEL)
}

fn zstd_decompress<B: AsRef<&[u8]>>(input: B, output: &mut Vec<u8>) -> std::io::Result<()>{
    zstd::stream::copy_decode(input, output)
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_zstd() {

    }
}
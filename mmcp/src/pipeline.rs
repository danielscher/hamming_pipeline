use async_std::{
    fs::{File, OpenOptions},
    io::BufReader,
    io::BufWriter,
    prelude::*,
};
use color_eyre::eyre::Result;

const BUF_SIZE: usize = 4096;

#[macro_export]
macro_rules! pipeline {
    ($encode:ident, $decode:ident) => {
        async fn pipeline_run(
            channel: &mut crate::channel::Channel,
        ) -> Result<(std::time::Duration, u32, u32)> {
            use async_std::prelude::*;
            use std::time::{Duration, Instant};

            let start = Instant::now();
            let stream = crate::pipeline::input().await?;
            let mut input_byte_count: u32 = 0;
            let stream = stream.map(|b| {
                input_byte_count += 1;
                b
            });
            let stream = $encode(stream).await?;
            let mut channel_byte_count: u32 = 0;
            let stream = stream.map(|b| {
                channel_byte_count += 1;
                b
            });
            let stream = channel.process(stream).await?;
            let stream = $decode(stream).await?;
            crate::pipeline::output(stream).await?;
            Ok((start.elapsed(), input_byte_count, channel_byte_count))
        }
    };
}

pub async fn input() -> Result<impl Stream<Item = u8>> {
    let file = BufReader::with_capacity(BUF_SIZE, File::open("resources/original.mp4").await?);
    Ok(file.bytes().map(|b| b.unwrap()))
}

pub async fn output<S>(mut stream: S) -> Result<()>
where
    S: Stream<Item = u8> + std::marker::Unpin,
{
    let mut output = BufWriter::with_capacity(
        BUF_SIZE,
        OpenOptions::new()
            .create(true)
            .write(true)
            .open("result.mp4")
            .await?,
    );
    while let Some(b) = stream.next().await {
        let buf = vec![b];
        output.write(&buf[..]).await?;
    }
    output.flush().await?;
    Ok(())
}

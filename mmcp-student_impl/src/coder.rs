use async_std::prelude::*;
use color_eyre::eyre::Result;

// encode message using hamming code process.
pub(super) async fn encode(
    mut stream: impl Stream<Item = u8> + Unpin,
) -> Result<impl Stream<Item = u8>> {
    let mut data = vec![];
    while let Some(byte) = stream.next().await {
        data.push(byte);
    }
    let data = encode_data(&data);
    let output = async_std::stream::from_iter(data);
    Ok(output)
}

pub(super) async fn decode(
    mut stream: impl Stream<Item = u8> + Unpin,
) -> Result<impl Stream<Item = u8>> {
    let mut data = vec![];
    while let Some(byte) = stream.next().await {
        data.push(byte);
    }
    let output = async_std::stream::from_iter(data);
    Ok(output)
}

fn encode_data(data: &[u8]) -> Vec<u8> {
    
    let mut segments = vec![];
    for byte in data {
        
        // calculate parity bits for upper 4 bits of the byte:
        // p1 = b0 ^ b1 ^ b3
        // p2 = b0 ^ b2 ^ b3
        // p4 = b1 ^ b2 ^ b3
        let p1 = (byte >> 7) & 1 ^ (byte >> 6) & 1 ^ (byte >> 4) & 1;
        let p2 = (byte >> 7) & 1 ^ (byte >> 5) & 1 ^ (byte >> 4) & 1;
        let p4 = (byte >> 6) & 1 ^ (byte >> 5) & 1 ^ (byte >> 4) & 1; 

        // encode the byte with parity bits.
        // p1-b0-p2-b1-b2-p4-b3-0
        let segment_up = ((byte & 0b1000_0000 ) << 6 ) | (byte >> 4) | (p1 << 7) | (p2 << 2) | (p4 << 1);
        segments.push(segment_up);

        // pairty for lower 4 bits of the byte:
        let p1 = (byte >> 3) & 1 ^ (byte >> 2) & 1 ^ byte  & 1;
        let p2 = (byte >> 3) & 1 ^ (byte >> 1) & 1 ^ byte & 1;
        let p4 = (byte >> 6) & 1 ^ (byte >> 1) & 1 ^ byte  & 1; 

        // encode the byte with parity bits.
        // p1-b4-p2-b5-b6-p4-b7-0
        let segment_low = (byte & 0b0000_1111) | ((byte & 0b0000_1000) << 3) | (p1 << 7) | (p2 << 2) | (p4 << 1);
        segments.push(segment_low);
    }

    interleave_segments(segments)

}


/* 
interleaves the data so that each byte consists of exactly 
one bit from each segment.

 we achieve interleaving by iterating over each index of a byte
 and for each index, we iterate over the segments and extract the bit
 until we have processed all the segments. 
 */

fn interleave_segments(segments: Vec<u8>) -> Vec<u8> {
    let mut interleave = vec![];
    let blocks = segments.len();
    let mut processed = 0;
    while processed < blocks {
        let mut interleaved_byte = 0b0000_0000;
        let mut count = 0b0u8;
        for i in 0..7 {

            // process 8 bytes at a time.
            for j in processed..blocks {
                interleaved_byte |= ((segments[j] >> (7-i)) & 1 ) << (7-count);
                count += 1;
                if count == 8 {
                    break;
                }
            }
            if count < 8 {
                break;
                todo!("handle incomplete byte")
            }           
            
        }
        processed += count as usize;
        interleave.push(interleaved_byte);
    }
    interleave
}
use std::error;

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
    let data = decode_data(&data);
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

    //interleave_segments(segments)
    interleave_segments(&mut segments)

}


/* 
interleaves the data so that each byte consists of exactly 
one bit from each segment.

 we achieve interleaving by iterating over each index of a byte
 and for each index, we iterate over the segments and extract the bit
 until we have processed all the segments. 
 */

fn interleave_segments(segments: &mut Vec<u8>) -> Vec<u8> {
    let mut interleave = vec![];
    let blocks = segments.len();

    // add padding to make the number of blocks a multiple of 8.
    if blocks % 8 != 0 {
        let padding = 8 - blocks % 8;
        for _ in 0..padding {
            interleave.push(0);
        }
    }


    let mut processed = 0;
    while processed < blocks {
        let mut interleaved_byte = 0b0000_0000;
        let mut count = 0b0u8;
        for i in 0..7 {

            // process 8 bytes at a time.
            for j in processed..blocks {
                interleaved_byte |= ((segments[j] >> (7-i)) & 1 ) << (7-count);
                count += 1;
                if count == 7 { // we have interleaved 8 bits
                    interleave.push(interleaved_byte);
                    interleaved_byte = 0b0000_0000;
                }
            }   
            
        }
        processed += 8;
        interleave.push(interleaved_byte);
    }
    interleave
}


fn decode_data(data: &[u8]) -> Vec<u8> {
    let mut decoded = vec![];
    for i in (0..data.len()).step_by(2) {
        let mut info_byte = 0;

        // decode the upper 4 bits of the byte.
        let upper_byte = data[i];
        let error_index = get_error_index(&upper_byte);
        if error_index != 0 {// check if error occured.
            let corrected_byte = upper_byte ^ (1 << error_index); // flip the bit
            let info_byte_upper = get_info_bits(&corrected_byte) << 4;
            info_byte |= info_byte_upper;
        } 
        else {
            info_byte |= get_info_bits(&upper_byte) << 4;
        }

        // decode the lower 4 bits of the byte.
        if i + 1 > data.len() {
            decoded.push(info_byte);
        }
        let lower_byte = data[i + 1];
        let error_index = get_error_index(&lower_byte);
        if error_index != 0 {// check if error occured.
            let corrected_byte = lower_byte ^ (1 << error_index); // flip the bit
            let info_byte_lower = get_info_bits(&corrected_byte);
            info_byte |= info_byte_lower;
        } 
        else {
            info_byte |= get_info_bits(&lower_byte);
        }
    }
    for byte in data {
        let encoded_byte = byte & 0b1111_1110;
        let error_index = get_error_index(&encoded_byte);
        if error_index != 0 {
            let corrected_byte = encoded_byte ^ (1 << error_index); // flip the bit

            decoded.push(corrected_byte);
        } else {
            decoded.push(encoded_byte);
        }
    }
    decoded
}

// performs xor of positions of bits set to 1.
fn get_error_index (byte: &u8) -> u8 {
    let mut error_index = 0;
    for i in 0..7 {
        if byte & (1 << i) != 0 {
            error_index ^= i;
        }
    }
    error_index
}

// info bits reside on indecies 2, 4, 5, 6.
//  always returns byte with infor bits at the rightmost position.
fn get_info_bits (byte: &u8) -> u8 {
    let mut info_byte = 0b0000_0000;
    info_byte |= (byte >> 5) & 1;
    info_byte |= (byte >> 3) & 1;
    info_byte |= (byte >> 2) & 1;
    info_byte |= (byte >> 1) & 1;
    info_byte
}

fn uninterleave_data(data: Vec<u8>) -> Vec<u8> {

    let mut uninterleaved = vec![];
    let blocks = data.len();
    let mut processed = 0;
    while processed < blocks {
        let mut uninterleaved_byte = 0b0000_0000;
        let mut count = 0b0u8;
        for i in 0..7 {

            for j in processed..blocks {
                uninterleaved_byte |= ((data[j] >> (7-i)) & 1 ) << (7-count);
                count += 1;
                if count == 7 { // we have interleaved 8 bits
                    uninterleaved.push(uninterleaved_byte);
                    uninterleaved_byte = 0b0000_0000;
                }
            }    
            
        }
        processed += count as usize;
        uninterleaved.push(uninterleaved_byte);
    }
    uninterleaved

}
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
    print!("original data: {:?}\n", &data[0..10]);
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
    println!("decoded data: {:?}", &data[0..10]);
    let output = async_std::stream::from_iter(data);
    Ok(output)
}

fn encode_data(data: &[u8]) -> Vec<u8> {
    let mut segments = vec![];
    for byte in data {
    
        // enumerating bits from left to right.
        // b0 = c3, b1 = c5, b2 = c6, b3 = c7
        let c3 = (byte & 0b1000_0000) >> 7;
        let c5 = (byte & 0b0100_0000) >> 6;
        let c6 = (byte & 0b0010_0000) >> 5;
        let c7 = (byte & 0b0001_0000) >> 4;


        // calculate parity bits for upper 4 bits of the byte:
        let p1 = c3 ^ c5 ^ c7;
        let p2 = c3 ^ c6 ^ c7;
        let p4 = c5 ^ c6 ^ c7;

        // encode the byte with parity bits.
        let segment_up = p1 << 7 | p2 << 6 | c3 << 5 | p4 << 4 | c5 << 3 | c6 << 2 | c7 << 1;
        //print!("upper bits: {}, segment: {}. ",(byte>>4), &segment_up);
        segments.push(segment_up);


        // extract info bits from lower 4 bits of the byte.
        let c3 = (byte & 0b0000_1000) >> 3;
        let c5 = (byte & 0b0000_0100) >> 2;
        let c6 = (byte & 0b0000_0010) >> 1;
        let c7 = byte & 0b0000_0001;

        // pairty for lower 4 bits of the byte:
        let p1 = c3 ^ c5 ^ c7;
        let p2 = c3 ^ c6 ^ c7;
        let p4 = c5 ^ c6 ^ c7; 

        // encode the byte with parity bits.
        let segment_low = p1 << 7 | p2 << 6 | c3 << 5 | p4 << 4 | c5 << 3 | c6 << 2 | c7 << 1;
        //print!("lower bits: {}, segment: {}. \n",(byte&15), &segment_low);
        segments.push(segment_low);
    }

    // interleave the segments.
    let interleave_encoded = interleave_segments(&mut segments);
    println!("interleaved: {:?}", &interleave_encoded[0..10]);
    interleave_encoded
}



// perform block interleaving on the segments.
fn interleave_segments(segments: &mut Vec<u8>) -> Vec<u8> {
    let mut interleaved_data = vec![];
    let bytes = segments.len();

    // add padding to make the number of bytes a multiple of 8.
    if bytes % 8 != 0 {
        add_padding(segments);
    }

    // block is 8 bytes long.
    for block in (0..bytes).step_by(8) {
        let interleaved_block = interleave_block(&segments[block..block + 8].to_vec());
        interleaved_data.extend(interleaved_block);
    }
    interleaved_data
}

// interleave 8 bytes of data.
fn interleave_block(block: &Vec<u8>) -> Vec<u8> {
    let mut interleave = vec![];
    let mut interleaved_byte = 0b0000_0000;
    let mut count = 0b0u8;
    for i in 0..8 {
        for byte in block.into_iter() {
            interleaved_byte |= ((byte >> (7-i)) & 1 ) << (7-count);
            count += 1;
            if count == 8 { // we have interleaved 8 bits
                interleave.push(interleaved_byte);
                interleaved_byte = 0b0000_0000;
                count = 0;
            }
        }    
    }
    interleave
}


fn decode_data(data: &[u8]) -> Vec<u8> {
    let deinterleaved = interleave_segments(&mut data.to_vec());
    println!("deinterleaved: {:?}", &deinterleaved[0..10]);
    let mut decoded = vec![];
    for i in (0..deinterleaved.len()).step_by(2) {
        let mut info_byte = 0;

        // decode the upper 4 bits of the byte.
        let upper_byte = deinterleaved[i];
        let error_index = get_error_index(&upper_byte);
        if error_index != 0 {// check if error occured.
            let corrected_byte = upper_byte ^ (1 << (8-error_index)); // flip the bit
            let info_byte_upper = get_info_bits(&corrected_byte) << 4;
            info_byte |= info_byte_upper;
        } 
        else {
            info_byte |= get_info_bits(&upper_byte) << 4;
        }

        // decode the lower 4 bits of the byte.
        if i + 1 > deinterleaved.len() {
            decoded.push(info_byte);
        }
        let lower_byte = deinterleaved[i + 1];
        let error_index = get_error_index(&lower_byte);
        if error_index != 0 {// check if error occured.
            let corrected_byte = lower_byte ^ (1 << error_index); // flip the bit
            let info_byte_lower = get_info_bits(&corrected_byte);
            info_byte |= info_byte_lower;
        } 
        else {
            info_byte |= get_info_bits(&lower_byte);
        }
        decoded.push(info_byte);
    }
    remove_padding(&mut decoded);
    decoded
}

// performs xor of positions of bits set to 1.
fn get_error_index (byte: &u8) -> u8 {
    let mut error_index = 0;
    for i in 1..8 {
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
    info_byte |= ((byte >> 5) & 1) << 3;
    info_byte |= ((byte >> 3) & 1) << 2;
    info_byte |= ((byte >> 2) & 1) << 1;
    info_byte |= (byte >> 1) & 1;
    info_byte
}

fn remove_padding(data: &mut Vec<u8>) {
    let padding = data.pop().unwrap() as usize;
    for _ in 0..padding {
        data.pop();
    }
}

fn add_padding(data: &mut Vec<u8>) {
    let padding = 8 - data.len() % 8;
    for i in 0..padding {
        data.push(i as u8);
    }
}

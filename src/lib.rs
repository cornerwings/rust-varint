#[cfg(test)]
extern crate quickcheck;
#[cfg(test)]
#[macro_use(quickcheck)]
extern crate quickcheck_macros;

use std::mem;

/*
 * Variable-length integer encoding.
 * This representation is idential to algorithm used by WiredTiger storage
 * engine. A nice property of this encoding scheme is that the packed representation
 * has the same lexicographic ordering as the integer values. 
 *
 * Refer to WiredTiger for the original algorithm.
 *
 * First byte  | Next |                        |
 * byte        | bytes| Min Value              | Max Value
 * ------------+------+------------------------+--------------------------------
 * [00 00xxxx] | free | N/A                    | N/A
 * [00 01llll] | llll | -2^64                  | -2^13 - 2^6
 * [00 1xxxxx] | 1    | -2^13 - 2^6            | -2^6 - 1
 * [01 xxxxxx] | 0    | -2^6                   | -1
 * [10 xxxxxx] | 0    | 0                      | 2^6 - 1
 * [11 0xxxxx] | 1    | 2^6                    | 2^13 + 2^6 - 1
 * [11 10llll] | llll | 2^13 + 2^6             | 2^64 - 1
 * [11 11xxxx] | free | N/A                    | N/A
 */
const NEG_MULTI_MARKER:u8 = 0x10;
const NEG_2BYTE_MARKER:u8 = 0x20;
const NEG_1BYTE_MARKER:u8 = 0x40;
const POS_1BYTE_MARKER:u8 = 0x80;
const POS_2BYTE_MARKER:u8 = 0xc0;
const POS_MULTI_MARKER:u8 = 0xe0;

const NEG_1BYTE_MIN:i64 = (-(1 << 6));
const NEG_2BYTE_MIN:i64 = (-(1 << 13) + NEG_1BYTE_MIN);
const POS_1BYTE_MAX:u64 = ((1 << 6) - 1);
const POS_2BYTE_MAX:u64 = ((1 << 13) + POS_1BYTE_MAX);

const INTPACK64_MAXSIZE:usize = mem::size_of::<u64>() + 1;

fn get_posint_bits(x: u64, start: usize, end: usize) -> u8 {
	return ((x & ((1u64 << (start)) - 1u64)) >> (end)) as u8
}

fn get_negint_bits(x: i64, start: usize, end: usize) -> u8 {
	return ((x & ((1i64 << (start)) - 1i64)) >> (end)) as u8
}

fn pack_posint_into(x: u64, res: &mut Vec<u8>) {

	let mut len = size_posint(x);
	let mut shift = (len - 1) << 3;

	res[0] |= (len & 0xf) as u8;
	let mut index = 1;

	loop {
		res[index] = (x >> shift) as u8;

		// update loop variable
	    shift -= 8;
	    index += 1;
	    len -= 1;

	    if len == 0 { break; }
	}

}

fn unpack_posint_from(res: &Vec<u8>) -> u64 {
	let mut x: u64 = 0;
	let mut len = res[0] & 0xf;
	let mut index = 1;

	loop {
		x = (x << 8) | res[index] as u64;
		index += 1;
		len -= 1;
		if len == 0 { break; }
	}

	return x;
}

fn pack_negint_into(x: i64, res: &mut Vec<u8>) {

	let lz = lz_negint(x);
	let mut len = size_negint(x);
	let mut shift = (len - 1) << 3;

	res[0] |= (lz & 0xf) as u8;
	let mut index = 1;

	loop {
		res[index] = (x >> shift) as u8;

		// update loop variable
	    shift -= 8;
	    index += 1;
	    len -= 1;

	    if len == 0 { break; }
	}

}

fn unpack_negint_from(res: &Vec<u8>) -> i64 {
	let mut len = mem::size_of::<u64>() as u8 - (res[0] & 0xf);
	let mut index = 1;
	let mut x: u64 = std::u64::MAX;

	loop {
		x = (x << 8) | res[index] as u64;
		index += 1;
		len -= 1;
		if len == 0 { break; }
	}

	unsafe { *(&x as *const u64 as *const i64) }
}

pub fn unpack_uint(res: &Vec<u8>) -> u64 {
	let mut x: u64;
	let marker = res[0] & 0xf0;

	if marker == POS_1BYTE_MARKER ||
	   marker == POS_1BYTE_MARKER | 0x10 || 
	   marker == POS_1BYTE_MARKER | 0x20 || 
	   marker == POS_1BYTE_MARKER | 0x30 {
		x = get_posint_bits(res[0] as u64, 6, 0) as u64;
	} 
	else if marker == POS_2BYTE_MARKER ||
			marker == (POS_2BYTE_MARKER | 0x10) {
		x = (get_posint_bits(res[0] as u64, 5, 0) as u64) << 8;
		x |= res[1] as u64;
		x += POS_1BYTE_MAX + 1;
	} else if marker == POS_MULTI_MARKER {
		x = unpack_posint_from(res);
		x += POS_2BYTE_MAX + 1;
	} else {
		unimplemented!()
	}

	return x;
}

pub fn unpack_int(res: &Vec<u8>) -> i64 {
	let mut x: i64;
	let marker = res[0] & 0xf0;

	if marker == NEG_MULTI_MARKER {
		x = unpack_negint_from(res);
	} 
	else if marker == NEG_2BYTE_MARKER ||
			marker == NEG_2BYTE_MARKER | 0x10 {
		x = (get_negint_bits(res[0] as i64, 5, 0) as i64) << 8;
		x |= res[1] as i64;
		x += NEG_2BYTE_MIN;
	} 
	else if marker == NEG_1BYTE_MARKER ||
	   		marker == NEG_1BYTE_MARKER | 0x10 || 
	   		marker == NEG_1BYTE_MARKER | 0x20 || 
	   		marker == NEG_1BYTE_MARKER | 0x30 {
		x = NEG_1BYTE_MIN + get_negint_bits(res[0] as i64, 6, 0) as i64;
	}
	else {
		let y = unpack_uint(res);
		x = unsafe { *(&y as *const u64 as *const i64) }
	}

	return x;
}

pub fn pack_uint(x: u64) -> Vec<u8> {

	let len = size_uint(x);
	let mut res: Vec<u8> = vec![0; len];

	if x <= POS_1BYTE_MAX {
		res[0] = POS_1BYTE_MARKER | get_posint_bits(x, 6, 0);
	}
	else if x <= POS_2BYTE_MAX {
		let mut y = x;
		y -= POS_1BYTE_MAX + 1;
		res[0] = POS_2BYTE_MARKER | get_posint_bits(y, 13, 8);
		res[1] = get_posint_bits(y, 8, 0);
	}
	else if x == POS_2BYTE_MAX + 1 {
		res[0] = POS_MULTI_MARKER | 0x1;
		res[1] = 0;
	}
	else {
		let mut y = x;
		y -= POS_2BYTE_MAX + 1;
		res[0] = POS_MULTI_MARKER;
		pack_posint_into(y, &mut res);
	}

	return res;
}

pub fn pack_int(x: i64) -> Vec<u8> {

	// Short circuit positive integer first
	if x >= 0 {
		return pack_uint(x as u64);
	}

	let len = size_int(x);
	let mut res: Vec<u8> = vec![0; len];

	if x < NEG_2BYTE_MIN {
		res[0] = NEG_MULTI_MARKER;
		pack_negint_into(x, &mut res);
	}
	else if x < NEG_1BYTE_MIN {
		let mut y = x;
		y -= NEG_2BYTE_MIN;
		res[0] = NEG_2BYTE_MARKER | get_negint_bits(y, 13, 8);
		res[1] = get_negint_bits(y, 8, 0);
	}
	else if x < 0 {
		let mut y = x;
		y -= NEG_1BYTE_MIN;
		res[0] = NEG_1BYTE_MARKER | get_negint_bits(y, 6, 0);
	}

	return res;
}

fn lz_posint(x: u64) -> usize {
	return if x == 0 {
		mem::size_of::<u64>()
	} else {
		(x.leading_zeros() >> 3) as usize
	};
}

fn lz_negint(x: i64) -> usize {
	return if !x == 0 {
		mem::size_of::<u64>()
	} else {
		(!x.leading_zeros() >> 3) as usize
	};
}

fn size_posint(x: u64) -> usize {
	return INTPACK64_MAXSIZE - lz_posint(x);
}


fn size_negint(x: i64) -> usize {
	return INTPACK64_MAXSIZE - lz_negint(x);
}


fn size_uint(x: u64) -> usize {
	if x <= POS_1BYTE_MAX {
		return 1;
	}
	if x <= POS_2BYTE_MAX + 1 {
		return 2;
	}

	let mut y = x;
	y -= POS_2BYTE_MAX + 1;
	return size_posint(y);
}

fn size_int(x: i64) -> usize {
	if x < NEG_2BYTE_MIN {
		return size_negint(x);
	}
	if x < NEG_1BYTE_MIN {
		return 2;
	}
	if x < 0 {
		return 1;
	}

	return size_uint(x as u64);
}


#[cfg(test)]
mod tests {
	use super::*;

    #[quickcheck]
    fn order_is_correct_pos(x: u64, y: u64) -> bool {
    	let xb = pack_uint(x);
    	let yb = pack_uint(y);

    	if x >= y {
    		xb >= yb
    	} else {
    		xb < yb
    	}
    }
	
    #[quickcheck]
    fn order_is_correct_neg(x: i64, y: i64) -> bool {
    	let xb = pack_int(x);
    	let yb = pack_int(y);

    	if x >= y {
    		xb >= yb
    	} else {
    		xb < yb
    	}
    }

    #[quickcheck]
    fn pack_and_unpack_pos(x: u64) -> bool {
    	let xb = pack_uint(x);
    	let y = unpack_uint(&xb);

    	x == y
    }

    #[quickcheck]
    fn pack_and_unpack_neg(x: i64) -> bool {
    	let xb = pack_int(x);
    	let y = unpack_int(&xb);

    	x == y
    }

}

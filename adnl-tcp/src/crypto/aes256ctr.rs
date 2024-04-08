use aes::cipher::generic_array::GenericArray;
use ctr::cipher::KeyIvInit;

pub(crate) type Aes256Ctr = ctr::Ctr128BE<aes::Aes256>;

pub(crate) fn build_cipher(shared_key: &[u8; 32], checksum: &[u8; 32]) -> Aes256Ctr {
    let x = shared_key;
    let y = checksum;

    let key = [
        x[ 0], x[ 1], x[ 2], x[ 3], x[ 4], x[ 5], x[ 6], x[ 7],
        x[ 8], x[ 9], x[10], x[11], x[12], x[13], x[14], x[15],
        y[16], y[17], y[18], y[19], y[20], y[21], y[22], y[23],
        y[24], y[25], y[26], y[27], y[28], y[29], y[30], y[31]
    ];
    let ctr = [
        y[ 0], y[ 1], y[ 2], y[ 3], x[20], x[21], x[22], x[23],
        x[24], x[25], x[26], x[27], x[28], x[29], x[30], x[31]
    ];

    Aes256Ctr::new(GenericArray::from_slice(&key), GenericArray::from_slice(&ctr))
}

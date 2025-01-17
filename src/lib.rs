#![cfg(target_os="android")]
#![allow(non_snake_case)]

mod fingerprinting {
    pub mod algorithm;
    mod hanning;
    pub mod signature_format;
}

use jni::objects::{JClass, JShortArray, JString};
use jni::JNIEnv;

use fingerprinting::algorithm::SignatureGenerator;

#[no_mangle]
pub extern "system" fn Java_com_alexmercerind_audire_native_ShazamSignature_create<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    input: JShortArray<'local>,
) -> JString<'local> {
    let size = env.get_array_length(&input).unwrap() as usize;
    let mut buffer = vec![0; size];
    env.get_short_array_region(&input, 0, &mut buffer).unwrap();
    let result = SignatureGenerator::make_signature_from_buffer(&buffer);
    return env.new_string(result.encode_to_uri().unwrap()).unwrap();
}

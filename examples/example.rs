use std::iter::Inspect;
use std::time::Instant;

use serde_json::Value;

fn main() {
    // let mut data = String::from(
    //     r#"
    //  {
    //      "name": /* full */ "John Doe",
    //      "age": 43,
    //      "phones": [
    //          "+44 1234567", // work phone
    //          "+44 2345678", // home phone
    //      ] /** comment **/
    //  }"#,
    // );
    // for i in 0u8..=255 {
    //     let ret = table::TABLE[i as usize];
    //     println!("{}: {}", i as char, ret);
    // }
    let start = Instant::now();
    let mut data = include_str!("../tsconfig.json").to_owned();
    for i in 0..10000 {
        let mut data = data.clone();
        json_strip_comments::strip(&mut data).unwrap();
    }
    dbg!(&start.elapsed());
}

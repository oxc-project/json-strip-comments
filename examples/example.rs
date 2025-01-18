use serde_json::Value;

fn main() {
    let mut data = String::from(
        r#"
     {
         "name": /* full */ "John Doe",
         "age": 43, # hash comment
         "phones": [
             "+44 1234567", // work phone
             "+44 2345678", // home phone
         ], /** comment **/
     }"#,
    );

    json_strip_comments::strip(&mut data).unwrap();
    let value: Value = serde_json::from_str(&data).unwrap();

    println!("{value}");
}

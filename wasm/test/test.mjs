import assert from 'assert';
import { strip } from '../../npm/json_strip_comments_wasm.js';

const s = `
{
     "name": /* full */ "John Doe",
     "age": 43, # hash line comment
     "phones": [
         "+44 1234567", // work phone
         "+44 2345678", // home phone
     ], /** comment **/
}`;

const expected = `
{
    "name": "John Doe",
    "age": 43,
    "phones": [
        "+44 1234567",
        "+44 2345678"
    ]
}`;

const stripped = strip(s);
assert.deepStrictEqual(JSON.parse(stripped), JSON.parse(expected));

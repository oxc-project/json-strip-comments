import json
# matches!(*self, b'\t' | b'\n' | b'\x0C' | b'\r' | b' ')
#


# return Ok(*c == b'}' || *c == b']');
#
# b'"' => InString,
# b'/' => {
#     *c = b' ';
#     InComment
# }
# b'#' if settings.hash_line_comments => {
#     *c = b' ';
#     InLineComment
# }
print(ord("\x0c"))

lst = []
for i in range(0, 255):
    ch = chr(i)
    # print(ch)
    #
    if i == 12:
        print(ch == "\x0c")
    match ch:
        case "\t" | "\n" | "\x0c" | "\r" | " ":
            lst.append(1)
        case '"':
            lst.append(2)
        case "/":
            lst.append(3)
        case "#":
            lst.append(4)
        case "}" | "]":
            lst.append(5)
        case _:
            lst.append(0)

with open("./result.json", "w") as f:
    json.dump(lst, f)

@external("https://repository.timot.se/test2", "return_arg")
declare function test2_return_arg(arg0: i32): i32;

export function return_arg(x: i32): i32 {
  return test2_return_arg(x)
}

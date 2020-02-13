@external("https://repository.timot.se/test1", "return_double_arg")
declare function return_double_arg(arg0: i32): i32;

export function return_arg(x: i32): i32 {
  return return_double_arg(x)
}
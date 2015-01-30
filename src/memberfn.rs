// Copyright (C) 2015 <Rick Richardson r@12sidedtech.com>
//
// This software may be modified and distributed under the terms
// of the MIT license.  See the LICENSE file for details.
#![feature(overloaded_calls)]

struct MemberFn0<'a, T, R> where T : 'a {
    fun: |&T|:'static -> R,
    obj: &'a T
}

impl<'a, T, R> MemberFn0<'a, T, R> where T : 'a {
    fn new(o: &'a T, f: |&T|:'static -> R) -> MemberFn0<'a, T, R> {
        MemberFn0 {
            fun: f,
            obj: o
        }
    }
    
    fn call(&mut self) -> R {
        (self.fun)(self.obj)
    }
}

struct MemberFn1<'a, T, R, A0> where T : 'a {
    fun: |&T, A0|:'static -> R,
    obj: &'a T
}

impl<'a, T, R, A0> MemberFn1<'a, T, R, A0> where T : 'a {
    fn new(o: &'a T, f: |&T, A0|:'static -> R) -> MemberFn1<'a, T, R, A0> {
        MemberFn1 {
            fun: f,
            obj: o
        }
    }
    
    fn call(&mut self, arg: A0) -> R {
        (self.fun)(self.obj, arg)
    }
}

struct MemberFn2<'a, T, R, A0, A1> where T : 'a {
    fun: |&T, A0, A1|:'static -> R,
    obj: &'a T
}

impl<'a, T, R, A0, A1> MemberFn2<'a, T, R, A0, A1> where T : 'a {
    fn new(o: &'a T, f: |&T, A0, A1|:'static -> R) -> MemberFn2<'a, T, R, A0, A1> {
        MemberFn2 {
            fun: f,
            obj: o
        }
    }
    
    fn call(&mut self, arg0: A0, arg1: A1) -> R {
        (self.fun)(self.obj, arg0, arg1)
    }
}

// there should be an Impl for FnMut, but it is currently broken, so we'll have to invoke with
// .call(...)

struct Foo {
  x: usize
}

impl Foo {

  fn winning(&self, a: usize) -> usize {
    a + self.x
  }

}

#[test]
fn main() {
  let bar = Foo { x : 10 };
  
  let mut binding = MemberFn1::new(&bar, |f: &Foo, a: usize| { f.winning(a) } ); 
  println!("result: {:?}", binding.call(10)); 
}

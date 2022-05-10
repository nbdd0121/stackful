use stackful::generator::*;
use std::pin::Pin;

fn main() {
    let mut gen = StackfulGenerator::new(|y: &YieldHandle<i32, i32>, mut r: i32| {
        for i in 0..100 {
            assert_eq!(r, i);
            r = y.yeet(i);
        }

        // Test yield cross nested generators.
        let mut gen2 = StackfulGenerator::new(|yy: &YieldHandle<(), ()>, ()| {
            assert_eq!(r, 100);
            r = y.yeet(100);
        });
        assert!(matches!(
            Pin::new(&mut gen2).resume(()),
            GeneratorState::Complete(())
        ),);
        drop(gen2);

        assert_eq!(r, 1000);
        1000
    });
    let mut gen = Pin::new(&mut gen);

    for i in 0..101 {
        println!("{:?}", gen.as_mut().resume(i));
    }
    assert!(matches!(
        gen.as_mut().resume(1000),
        GeneratorState::Complete(1000)
    ));
}

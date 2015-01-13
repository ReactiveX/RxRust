
This is an implementation of reactive streams, which, at the high level, is patterned off of the interfaces and
protocols defined in http://reactive-streams.org. 

It also draws a lot of inspiration from clojures Transducers, in that it attempts to decouple the functions doing the
work from their plumbing. It does this with Strategies. These effect how a publisher/processor will consume and output
data. 
    
The high level API built on top of the interfaces (the customer facing portions) draw inspiration more from Elm than the java based Rx libraries. 

The core types are as follows: 

```
Producer<Strategy, Output> 
```

Producer is where most of the action happens.  It generates data and supplies a datum at a time to its subscribers via
the Subscriber::on_next method. If it has multiple subscribers, how it supplies each subscriber with data is up to the
Strategy.  It also limits its output using a pushback mechanism by being supplied a request amount by the subscriber
(via its subscription). The hope is that a producer can be as simple as a proc, relying on the strategy and the
subscription objects to do most of its heavy lifting, while the proc does only data transformation/generation.

```
OutputStrategy<Output>
```

There are many ways a producer might distribute data.  When request limiting is present, there are a few options a
strategy might take: 
1. Keep all subscribers in-sync by broadcasting to all only when they all have remaining requests. 
2. Eagerly send to any subscribers with available requests, discarding the messages to those without. 
3. Round-Robin across subscribers with available requests. 
4. Keep a queue for each subscriber which is currently without requests, in an attempt to keep all subscribers in sync. 

```
InputStrategy<...>
```

An input Strategy is what copes with various types of input, Iterators, data structures, multiple subscriptions, etc. It
can choose to execute one when it receives the other, or selects, or anything, really. 


```
Subscriber<Input>
```

The subscriber subscribes to a Producer. Its only responsibility is to supply to the producer a reqeust number (via the subscription
object) which acts as a backpressure mechanism. 

```
Processor<Strategy, Input, Output> : Producer<Strategy, Output> + Subscriber<Input>
```

A processor is simply a Producer + a Subscriber. It is a link in the execution chain.  Among its other responsibilities,
it must pass down request backpressure values from its subscribers.  It would also propagate errors downstream, and
propagate unsubscribes upstream. 



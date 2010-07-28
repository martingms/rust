io fn main() -> () {
   test00();
}

io fn test00_start(chan[int] c, int start, int number_of_messages) {
    let int i = 0;
    while (i < number_of_messages) {
        c <| start + i;
        i += 1;
    }    
}

io fn test00() {
    let int r = 0;    
    let int sum = 0;
    let port[int] p = port();
    let int number_of_messages = 10;
        
    let task t0 = spawn test00_start(chan(p), 
                        number_of_messages * 0, number_of_messages);
    let task t1 = spawn test00_start(chan(p), 
                        number_of_messages * 1, number_of_messages);
    let task t2 = spawn test00_start(chan(p), 
                        number_of_messages * 2, number_of_messages);
    let task t3 = spawn test00_start(chan(p), 
                        number_of_messages * 3, number_of_messages);
    
    let int i = 0;
    while (i < number_of_messages) {
        r <- p; sum += r;
        r <- p; sum += r;
        r <- p; sum += r;
        r <- p; sum += r;
        i += 1;
    }
            
    join t0;
    join t1;
    join t2;
    join t3;
    
    check (sum == (((number_of_messages * 4) * 
                   ((number_of_messages * 4) - 1)) / 2));
}
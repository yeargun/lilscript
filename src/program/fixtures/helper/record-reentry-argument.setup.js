let readState,writeState;function keep(r,w){readState=r;writeState=w}function argument(){console.log("before:"+readState());writeState(10);console.log("after:"+readState());return 3}

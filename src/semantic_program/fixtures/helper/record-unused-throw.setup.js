let readState,writeState;function keep(r,w){readState=r;writeState=w}function argument(){writeState(10);console.log("before-throw:"+readState());throw Error("stop")}

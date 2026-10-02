globalThis.convert=(packet,later)=>{
  console.log('host:'+packet.inner.value+':'+later);
  packet.inner.value=7;
  packet.shared.push(8);
  return {get inner(){console.log('inner getter');return {get value(){console.log('value getter');return 42;}};},shared:packet.shared};
};

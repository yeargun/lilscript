export function metadata(value,name,length) {
 if(typeof value!=="function")return;
 Object.defineProperty(value,"name",{value:name,configurable:true});
 if(length>=0)Object.defineProperty(value,"length",{value:length,configurable:true});
}
export function methodMetadata(value,key,name,length){metadata(value[key],name,length)}

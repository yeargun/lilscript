#include <stdbool.h>
#include <ctype.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static inline int32_t lilscript_idiv(int32_t a,int32_t b){if(!b)return 0;return a==INT32_MIN&&b==-1?INT32_MIN:a/b;}
static inline int32_t lilscript_irem(int32_t a,int32_t b){if(!b)return 0;return a==INT32_MIN&&b==-1?0:a%b;}
typedef struct LilScriptArrayHeader{void*data;int32_t len,cap;}*LilScriptArray;typedef struct{void*fn;void*env;}LilScriptClosure;typedef char* LilScriptString;
static inline LilScriptArray lilscript_array(int32_t n,size_t z){LilScriptArray a=malloc(sizeof*a);if(!a)abort();a->data=calloc((size_t)n,z);a->len=a->cap=n;if(n&&!a->data)abort();return a;}
static inline void*lilscript_push(LilScriptArray a,size_t z){if(a->len==a->cap){a->cap=a->cap?a->cap*2:4;a->data=realloc(a->data,(size_t)a->cap*z);if(!a->data)abort();}return(char*)a->data+(size_t)a->len++*z;}
static inline void*lilscript_pop(LilScriptArray a,size_t z){if(!a->len)abort();return(char*)a->data+(size_t)--a->len*z;}
static inline LilScriptString lilscript_dup(const char*s){size_t n=strlen(s)+1;char*r=malloc(n);if(!r)abort();memcpy(r,s,n);return r;}
static inline LilScriptString lilscript_cat(const char*a,const char*b){size_t x=strlen(a),y=strlen(b);char*r=malloc(x+y+1);if(!r)abort();memcpy(r,a,x);memcpy(r+x,b,y+1);return r;}
static inline LilScriptString lilscript_i32(int32_t v){char b[16];snprintf(b,sizeof b,"%d",v);return lilscript_dup(b);}
static inline LilScriptString lilscript_f64(double v){char b[32];snprintf(b,sizeof b,"%.17g",v);return lilscript_dup(b);}
static inline bool lilscript_ends(const char*s,const char*x){size_t a=strlen(s),b=strlen(x);return a>=b&&!memcmp(s+a-b,x,b);}
static inline LilScriptString lilscript_case(const char*s,bool upper){LilScriptString r=lilscript_dup(s);for(char*p=r;*p;p++)*p=(char)(upper?toupper((unsigned char)*p):tolower((unsigned char)*p));return r;}
int main(void){int32_t v0;int32_t v11;LilScriptArray v6;int32_t v19;int32_t v4;int32_t v2;int32_t v22;int32_t v23;const char* v9;int32_t v16;int32_t v14;int32_t v21;int32_t v12;int32_t v18;int32_t v5;int32_t v1;bool v15;int32_t v17;int32_t v20;int32_t v13;uint32_t s=0;for(;;)switch(s){case 0:{v0=((int32_t)3);v1=((int32_t)1);v2=((int32_t)4);v4=((int32_t)5);v5=((int32_t)9);v6=lilscript_array(6,sizeof(int32_t));((int32_t*)v6->data)[0]=v0;((int32_t*)v6->data)[1]=v1;((int32_t*)v6->data)[2]=v2;((int32_t*)v6->data)[3]=v1;((int32_t*)v6->data)[4]=v4;((int32_t*)v6->data)[5]=v5;s=1;continue;}case 1:{v11=((int32_t)0);int32_t p0=v11;int32_t p1=v11;v12=p0;v13=p1;s=2;continue;}case 2:{v14=((int32_t)6);v15=(v13<v14);if(v15){s=3;}else{s=5;}continue;}case 3:{v16=((int32_t*)v6->data)[v13];v17=((int32_t)3);v18=(int32_t)((uint32_t)v13+(uint32_t)v17);v19=(int32_t)((uint32_t)v16*(uint32_t)v18);v20=(int32_t)((uint32_t)v19+(uint32_t)v13);v21=(int32_t)((uint32_t)v12+(uint32_t)v20);s=4;continue;}case 4:{v22=((int32_t)1);v23=(int32_t)((uint32_t)v13+(uint32_t)v22);int32_t p2=v21;int32_t p3=v23;v12=p2;v13=p3;s=2;continue;}case 5:{s=6;continue;}case 6:{v9=lilscript_dup("");v9=lilscript_cat(v9,"checksum=");v9=lilscript_cat(v9,lilscript_i32(v12));puts(v9);return 0;}}return 0;}

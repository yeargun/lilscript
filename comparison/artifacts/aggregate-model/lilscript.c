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
typedef struct LilScriptStruct_506f696e74 LilScriptStruct_506f696e74;
typedef struct LilScriptStruct_52656374616e676c65 LilScriptStruct_52656374616e676c65;
typedef struct LilScriptClass_4d6f64656c436f756e746572*LilScriptClass_4d6f64656c436f756e746572;
struct LilScriptStruct_506f696e74{int32_t f0;int32_t f1;};
struct LilScriptStruct_52656374616e676c65{LilScriptStruct_506f696e74 f0;int32_t f1;int32_t f2;};
struct LilScriptClass_4d6f64656c436f756e746572{int32_t f0;};
int main(void){int32_t v29;int32_t v0;int32_t v1;LilScriptStruct_506f696e74 v2;int32_t v8;int32_t v3;int32_t v12;int32_t v31;int32_t v11;int32_t v4;int32_t v34;uint32_t s=0;for(;;)switch(s){case 0:{v0=((int32_t)3);v1=((int32_t)4);v2=(LilScriptStruct_506f696e74){v0,v1};v3=((int32_t)6);v4=((int32_t)7);v8=v2.f0;v11=v2.f1;v12=(int32_t)((uint32_t)v8+(uint32_t)v11);v29=((int32_t)42);printf("%d\n",v29);v31=(int32_t)((uint32_t)v12+(uint32_t)v3);printf("%d\n",v31);v34=(int32_t)((uint32_t)v31+(uint32_t)v4);printf("%d\n",v34);return 0;}}return 0;}

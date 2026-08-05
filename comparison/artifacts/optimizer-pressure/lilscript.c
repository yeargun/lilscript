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
typedef struct LilScriptClass_426f78*LilScriptClass_426f78;
struct LilScriptClass_426f78{int32_t f0;};
int main(void){const char* v13;int32_t v27;int32_t v36;int32_t v28;uint32_t s=0;for(;;)switch(s){case 0:{v36=((int32_t)4);printf("%d\n",v36);v27=((int32_t)42);v28=((int32_t)84);printf("%d\n",v28);printf("%d\n",v27);v13="application-build-identifier";puts(v13);puts(v13);puts(v13);s=1;continue;}case 1:{s=2;continue;}case 2:{return 0;}}return 0;}

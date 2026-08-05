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
static int32_t f1(void*,int32_t);
static bool f2(void*,int32_t);
static int32_t f3(void*,int32_t,int32_t);
static void f4(void*,int32_t);
static int32_t f1(void* env,int32_t v0){int32_t v2;int32_t v3;(void)env;uint32_t s=0;for(;;)switch(s){case 0:{v2=((int32_t)3);v3=(int32_t)((uint32_t)v0*(uint32_t)v2);return v3;}}}
static bool f2(void* env,int32_t v0){int32_t v3;int32_t v4;int32_t v2;bool v5;(void)env;uint32_t s=0;for(;;)switch(s){case 0:{v2=((int32_t)2);v3=lilscript_irem(v0,v2);v4=((int32_t)0);v5=(v3==v4);return v5;}}}
static int32_t f3(void* env,int32_t v0,int32_t v1){int32_t v4;(void)env;uint32_t s=0;for(;;)switch(s){case 0:{v4=(int32_t)((uint32_t)v0+(uint32_t)v1);return v4;}}}
static void f4(void* env,int32_t v0){(void)env;uint32_t s=0;for(;;)switch(s){case 0:{printf("%d\n",v0);return;}}}
int main(void){int32_t v6;LilScriptClosure v9;LilScriptArray v7;LilScriptArray v10;int32_t v0;int32_t v1;int32_t v2;LilScriptClosure v15;int32_t v5;LilScriptClosure v12;int32_t v16;LilScriptArray v13;int32_t v17;LilScriptClosure v19;int32_t v4;uint32_t s=0;for(;;)switch(s){case 0:{v0=((int32_t)3);v1=((int32_t)1);v2=((int32_t)2);v4=((int32_t)4);v5=((int32_t)5);v6=((int32_t)6);v7=lilscript_array(6,sizeof(int32_t));((int32_t*)v7->data)[0]=v1;((int32_t*)v7->data)[1]=v2;((int32_t*)v7->data)[2]=v0;((int32_t*)v7->data)[3]=v4;((int32_t*)v7->data)[4]=v5;((int32_t*)v7->data)[5]=v6;v9=(LilScriptClosure){(void*)f1,NULL};v10=lilscript_array(v7->len,sizeof(int32_t));for(int32_t i10=0;i10<v7->len;i10++){((int32_t*)v10->data)[i10]=(((int32_t(*)(void*,int32_t))v9.fn)(v9.env,((int32_t*)v7->data)[i10]));}v12=(LilScriptClosure){(void*)f2,NULL};v13=lilscript_array(v10->len,sizeof(int32_t));v13->len=0;for(int32_t i13=0;i13<v10->len;i13++){if((((bool(*)(void*,int32_t))v12.fn)(v12.env,((int32_t*)v10->data)[i13])))((int32_t*)v13->data)[v13->len++]=((int32_t*)v10->data)[i13];}v15=(LilScriptClosure){(void*)f3,NULL};v16=((int32_t)0);v17=v16;for(int32_t i17=0;i17<v13->len;i17++){v17=(((int32_t(*)(void*,int32_t,int32_t))v15.fn)(v15.env,v17,((int32_t*)v13->data)[i17]));}v19=(LilScriptClosure){(void*)f4,NULL};for(int32_t i19=0;i19<v13->len;i19++){(((void(*)(void*,int32_t))v19.fn)(v19.env,((int32_t*)v13->data)[i19]));}printf("%d\n",v17);return 0;}}return 0;}

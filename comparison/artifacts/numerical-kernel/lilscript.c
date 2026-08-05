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
static int32_t f1(int32_t);
static int32_t f1(int32_t v0){int32_t v2;int32_t v9;int32_t v10;int32_t v4;int32_t v8;bool v3;int32_t v7;uint32_t s=0;for(;;)switch(s){case 0:{v2=((int32_t)1);v3=(v0<=v2);if(v3){s=1;}else{s=2;}continue;}case 1:{v4=((int32_t)1);return v4;}case 2:{s=3;continue;}case 3:{v7=((int32_t)1);v8=(int32_t)((uint32_t)v0-(uint32_t)v7);v9=f1(v8);v10=(int32_t)((uint32_t)v0*(uint32_t)v9);return v10;}}}
int main(void){int32_t v7;int32_t v14;int32_t v11;bool v13;int32_t v12;int32_t v10;int32_t v17;int32_t v15;bool v20;int32_t v3;int32_t v22;int32_t v23;int32_t v4;int32_t v18;int32_t v16;int32_t v19;int32_t v0;int32_t v1;int32_t v21;uint32_t s=0;for(;;)switch(s){case 0:{v0=((int32_t)7);v1=f1(v0);printf("%d\n",v1);v3=((int32_t)1071);v4=((int32_t)462);s=1;continue;}case 1:{int32_t p0=v3;int32_t p1=v4;v10=p0;v11=p1;s=2;continue;}case 2:{v12=((int32_t)0);v13=(v11!=v12);if(v13){s=3;}else{s=4;}continue;}case 3:{v14=lilscript_irem(v10,v11);int32_t p2=v11;int32_t p3=v14;v10=p2;v11=p3;s=2;continue;}case 4:{s=5;continue;}case 5:{printf("%d\n",v10);v7=((int32_t)12);s=6;continue;}case 6:{v15=((int32_t)0);v16=((int32_t)1);int32_t p4=v15;int32_t p5=v16;int32_t p6=v15;v17=p4;v18=p5;v19=p6;s=7;continue;}case 7:{v20=(v19<v7);if(v20){s=8;}else{s=10;}continue;}case 8:{v21=(int32_t)((uint32_t)v17+(uint32_t)v18);s=9;continue;}case 9:{v22=((int32_t)1);v23=(int32_t)((uint32_t)v19+(uint32_t)v22);int32_t p7=v18;int32_t p8=v21;int32_t p9=v23;v17=p7;v18=p8;v19=p9;s=7;continue;}case 10:{s=11;continue;}case 11:{printf("%d\n",v17);return 0;}}return 0;}

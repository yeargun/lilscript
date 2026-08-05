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
int main(void){int32_t v27;int32_t v3;int32_t v13;int32_t v30;int32_t v4;int32_t v8;bool v10;bool v18;int32_t v17;int32_t v24;int32_t v9;int32_t v7;bool v6;bool v23;int32_t v25;int32_t v22;int32_t v0;int32_t v19;int32_t v12;int32_t v26;int32_t v16;int32_t v20;int32_t v14;int32_t v28;int32_t v21;int32_t v29;int32_t v15;int32_t v11;int32_t v5;uint32_t s=0;for(;;)switch(s){case 0:{v0=((int32_t)12);s=1;continue;}case 1:{v3=((int32_t)0);int32_t p0=v3;int32_t p1=v3;v4=p0;v5=p1;s=2;continue;}case 2:{v6=(v5<v0);if(v6){s=3;}else{s=5;}continue;}case 3:{v7=((int32_t)3);v8=lilscript_irem(v5,v7);v9=((int32_t)0);v10=(v8==v9);if(v10){s=6;}else{s=7;}continue;}case 4:{v12=((int32_t)1);v13=(int32_t)((uint32_t)v5+(uint32_t)v12);int32_t p2=v11;int32_t p3=v13;v4=p2;v5=p3;s=2;continue;}case 5:{s=15;continue;}case 6:{int32_t p4=v4;v11=p4;s=4;continue;}case 7:{s=8;continue;}case 8:{v14=((int32_t)0);int32_t p5=v4;int32_t p6=v14;v15=p5;v16=p6;s=9;continue;}case 9:{v17=((int32_t)4);v18=(v16<v17);if(v18){s=10;}else{s=11;}continue;}case 10:{v19=(int32_t)((uint32_t)v5+(uint32_t)v16);v20=((int32_t)2);v21=lilscript_irem(v19,v20);v22=((int32_t)0);v23=(v21==v22);if(v23){s=12;}else{s=13;}continue;}case 11:{int32_t p7=v15;v11=p7;s=4;continue;}case 12:{v24=(int32_t)((uint32_t)v5*(uint32_t)v16);v25=(int32_t)((uint32_t)v15+(uint32_t)v24);int32_t p8=v25;v28=p8;s=14;continue;}case 13:{v26=((int32_t)1);v27=(int32_t)((uint32_t)v15+(uint32_t)v26);int32_t p9=v27;v28=p9;s=14;continue;}case 14:{v29=((int32_t)1);v30=(int32_t)((uint32_t)v16+(uint32_t)v29);int32_t p10=v28;int32_t p11=v30;v15=p10;v16=p11;s=9;continue;}case 15:{printf("%d\n",v4);return 0;}}return 0;}

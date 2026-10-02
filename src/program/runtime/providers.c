/* Explicit C11 host services. Text boundaries use strict UTF-8: malformed
   bytes/surrogates raise EncodingError, rather than silently changing paths,
   arguments or file contents. Embedded NUL in a path/key is rejected. */
#include <time.h>
static LS_NATIVE_UNUSED ls_string ls_host_utf8(const char *bytes,size_t length) {
    ls_string text=ls_string_allocate(length);uint16_t *out=(uint16_t *)text.data;size_t count=0;
    for(size_t i=0;i<length;) {
        uint32_t code=(unsigned char)bytes[i++];unsigned more=0;uint32_t minimum=0;
        if(code<0x80) {}
        else if(code>=0xc2 && code<=0xdf) {code&=0x1f;more=1;minimum=0x80;}
        else if(code>=0xe0 && code<=0xef) {code&=0x0f;more=2;minimum=0x800;}
        else if(code>=0xf0 && code<=0xf4) {code&=7;more=3;minimum=0x10000;}
        else goto invalid;
        if(length-i<more) goto invalid;
        for(unsigned n=0;n<more;++n) {unsigned byte=(unsigned char)bytes[i++];if((byte&0xc0)!=0x80) goto invalid;code=(code<<6)|(byte&63);}
        if(code<minimum || code>0x10ffff || (code>=0xd800 && code<=0xdfff)) goto invalid;
        if(code>0xffff) {code-=0x10000;out[count++]=(uint16_t)(0xd800+(code>>10));out[count++]=(uint16_t)(0xdc00+(code&1023));}
        else out[count++]=(uint16_t)code;
    }
    text.length=count;return text;
invalid:
    ls_string_release(text);ls_native_raise_error("EncodingError","host text is not valid UTF-8");return (ls_string){0};
}
static LS_NATIVE_UNUSED char *ls_host_bytes(ls_string text,bool name,size_t *length) {
    if(text.length>(SIZE_MAX-1)/3) ls_native_resource_failure();
    char *out=malloc(text.length*3+1);if(!out) ls_native_resource_failure();size_t count=0;
    for(size_t i=0;i<text.length;++i) {
        uint32_t code=text.data[i];
        if(name && !code) {free(out);ls_native_raise_error("TypeError","host path or environment name contains NUL");return NULL;}
        if(code>=0xd800 && code<=0xdbff && i+1<text.length && text.data[i+1]>=0xdc00 && text.data[i+1]<=0xdfff) {code=0x10000+((code-0xd800)<<10)+(text.data[++i]-0xdc00);}
        else if(code>=0xd800 && code<=0xdfff) {free(out);ls_native_raise_error("EncodingError","host text contains an unpaired UTF-16 surrogate");return NULL;}
        if(code<0x80) out[count++]=(char)code;
        else if(code<0x800) {out[count++]=(char)(0xc0|(code>>6));out[count++]=(char)(0x80|(code&63));}
        else if(code<0x10000) {out[count++]=(char)(0xe0|(code>>12));out[count++]=(char)(0x80|((code>>6)&63));out[count++]=(char)(0x80|(code&63));}
        else {out[count++]=(char)(0xf0|(code>>18));out[count++]=(char)(0x80|((code>>12)&63));out[count++]=(char)(0x80|((code>>6)&63));out[count++]=(char)(0x80|(code&63));}
    }
    out[count]=0;*length=count;return out;
}
static LS_NATIVE_UNUSED void ls_host_write(FILE *file,ls_string text) {
    size_t length;char *bytes=ls_host_bytes(text,false,&length);if(!bytes) return;
    bool failed=fwrite(bytes,1,length,file)!=length;free(bytes);
    if(fflush(file)!=0) failed=true;
    if(failed) ls_native_raise_error("IOError","host stream write failed");
}
#if defined(LS_HOST_PROVIDER_0) || defined(LS_HOST_PROVIDER_1)
static ls_string *ls_host_argv;static int ls_host_argc;
static void ls_host_arguments_clear(void) {
    for(int i=0;i<ls_host_argc;++i) ls_string_release(ls_host_argv[i]);
    free(ls_host_argv);ls_host_argv=NULL;ls_host_argc=0;
}
static bool ls_host_arguments_init(int argc,const char *const *argv) {
    if(argc<0 || (argc && !argv)) {ls_native_raise_error("TypeError","invalid native process arguments");return false;}
    if(!argc) return true;
    ls_host_argv=calloc((size_t)argc,sizeof *ls_host_argv);if(!ls_host_argv) ls_native_resource_failure();
    for(int i=0;i<argc;++i) {
        if(!argv[i]) {ls_native_raise_error("TypeError","null native process argument");return false;}
        ls_host_argv[ls_host_argc++]=ls_host_utf8(argv[i],strlen(argv[i]));if(ls_native_raised) return false;
    }
    return true;
}
#endif
#ifdef LS_HOST_PROVIDER_0
int32_t host_lil_arg_count(void) {return ls_host_argc;}
#endif
#ifdef LS_HOST_PROVIDER_1
ls_value host_lil_arg(int32_t index) {
    return index<0 || index>=ls_host_argc ? (ls_value){0} : ls_value_string(ls_string_hold(ls_host_argv[index]));
}
#endif
#ifdef LS_HOST_PROVIDER_2
ls_value host_lil_env(ls_string name) {
    size_t length;char *key=ls_host_bytes(name,true,&length);if(!key) return (ls_value){0};
    const char *value=getenv(key);free(key);if(!value) return (ls_value){0};
    return ls_value_string(ls_host_utf8(value,strlen(value)));
}
#endif
#ifdef LS_HOST_PROVIDER_3
void host_lil_stdout(ls_string text) {ls_host_write(stdout,text);}
#endif
#ifdef LS_HOST_PROVIDER_4
void host_lil_stderr(ls_string text) {ls_host_write(stderr,text);}
#endif
#if defined(LS_HOST_PROVIDER_5) || defined(LS_HOST_PROVIDER_10)
static LS_NATIVE_UNUSED ls_string ls_host_read(FILE *file,bool close) {
    char *bytes=NULL;size_t length=0,capacity=0;bool failed=false;
    for(;;) {
        if(capacity-length<4096) {
            if(length>(size_t)INT32_MAX-4096) {failed=true;break;}
            size_t next=capacity ? capacity+capacity/2+4096 : 4096;
            if(next>(size_t)INT32_MAX) next=INT32_MAX;
            char *grown=realloc(bytes,next);if(!grown) ls_native_resource_failure();bytes=grown;capacity=next;
        }
        size_t count=fread(bytes+length,1,capacity-length,file);length+=count;
        if(ferror(file)) {failed=true;break;}if(feof(file)) break;
    }
    if(close && fclose(file)!=0) failed=true;
    if(failed) {free(bytes);ls_native_raise_error("IOError","host file read failed or exceeds the text limit");return (ls_string){0};}
    ls_string result=ls_host_utf8(bytes,length);free(bytes);return result;
}
#endif
#ifdef LS_HOST_PROVIDER_5
ls_string host_lil_read_text(ls_string path) {
    size_t unused;char *name=ls_host_bytes(path,true,&unused);if(!name) return (ls_string){0};
    FILE *file=fopen(name,"rb");free(name);if(!file) {ls_native_raise_error("IOError","cannot open host file for reading");return (ls_string){0};}
    return ls_host_read(file,true);
}
#endif
#ifdef LS_HOST_PROVIDER_10
ls_string host_lil_stdin(void) {return ls_host_read(stdin,false);}
#endif
#ifdef LS_HOST_PROVIDER_6
void host_lil_write_text(ls_string path,ls_string text) {
    size_t unused;char *name=ls_host_bytes(path,true,&unused);if(!name) return;
    /* Validate the payload before truncating a destination. */
    size_t length;char *bytes=ls_host_bytes(text,false,&length);if(!bytes) {free(name);return;}
    FILE *file=fopen(name,"wb");free(name);
    if(!file) {free(bytes);ls_native_raise_error("IOError","cannot open host file for writing");return;}
    bool failed=fwrite(bytes,1,length,file)!=length;free(bytes);
    if(fclose(file)!=0) failed=true;
    if(failed) ls_native_raise_error("IOError","host file write failed");
}
#endif
#ifdef LS_HOST_PROVIDER_7
double host_lil_wall_time(void) {
    struct timespec now;if(timespec_get(&now,TIME_UTC)!=TIME_UTC) {ls_native_raise_error("IOError","host wall clock unavailable");return 0;}
    return (double)now.tv_sec+(double)now.tv_nsec/1e9;
}
#endif
#ifdef LS_HOST_PROVIDER_8
double host_lil_cpu_time(void) {
    clock_t now=clock();if(now==(clock_t)-1) {ls_native_raise_error("IOError","host CPU clock unavailable");return 0;}
    return (double)now/(double)CLOCKS_PER_SEC;
}
#endif
#ifdef LS_HOST_PROVIDER_9
void host_lil_exit(int32_t status) {fflush(NULL);_Exit(status);}
#endif

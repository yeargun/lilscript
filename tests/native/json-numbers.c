/* Compare with json-numbers.oracle.mjs beside a generated JSON fixture. */
#define main ls_json_fixture_main
#include "ownership.c"
#undef main
#include <locale.h>
static uint32_t ls_sample_state=UINT32_C(0x31415926);
static uint32_t ls_sample_word(void) {
    uint32_t x=ls_sample_state; x^=x<<13; x^=x>>17; x^=x<<5; return ls_sample_state=x;
}
static void ls_sample_format(uint64_t bits) {
    double value; memcpy(&value,&bits,sizeof value);
    ls_string text=ls_number_to_string(value); ls_print_string(text); ls_string_release(text);
}
static void ls_sample_parse(const char *decimal,size_t length) {
    uint16_t units[4096]; assert(length<sizeof units/sizeof *units);
    for(size_t j=0;j<length;j++) units[j]=(unsigned char)decimal[j];
    ls_value parsed=ls_json_parse((ls_string){units,length,NULL}); assert(!ls_native_raised);
    double number=ls_value_to_number(parsed); uint64_t bits; memcpy(&bits,&number,sizeof bits);
    printf("%016" PRIx64 "\n",bits); ls_value_release(parsed);
}
int main(int argc,char **argv) {
    if(!ls_runtime_init()) return 1;
    if(argc>1 && !setlocale(LC_NUMERIC,argv[1])) return 2;
    for(uint64_t exponent=0;exponent<2048;exponent++) {
        uint64_t bits=exponent<<52;
        ls_sample_format(bits); ls_sample_format(bits|UINT64_C(0x8000000000000000));
        if(bits) ls_sample_format(bits-1);
        ls_sample_format(bits+1);
    }
    for(int i=0;i<100000;i++) {
        uint64_t high=ls_sample_word(),low=ls_sample_word();
        ls_sample_format((high<<32)|low);
    }
    for(int i=0;i<100000;i++) {
        uint64_t high=ls_sample_word(),low=ls_sample_word(),mantissa=(high<<32)|low;
        int exponent=(int)(ls_sample_word()%801)-400;
        char decimal[64]; int length=snprintf(decimal,sizeof decimal,"%s%" PRIu64 "e%d",i&1?"-":"",mantissa,exponent);
        ls_sample_parse(decimal,(size_t)length);
    }
    char decimal[4096];
    while(fgets(decimal,sizeof decimal,stdin)) {
        size_t length=strlen(decimal); assert(length && decimal[length-1]=='\n');
        ls_sample_parse(decimal,length-1);
    }
    ls_native_collect_cycles(); assert(ls_native_owned_objects()==0);
    return ferror(stdout)?1:0;
}

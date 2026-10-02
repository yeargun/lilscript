/* Compile beside a generated text fixture's ownership.c / host.c. Compare
   this canonical binary stream with text-unicode.oracle.mjs (Unicode 17).
   Every code point, including isolated surrogate units, is converted both ways. */
#define main ls_text_fixture_main
#include "ownership.c"
#undef main
int main(void) {
    if(!ls_runtime_init()) return 1;
    for(uint32_t point=0;point<=0x10FFFF;point++) {
        uint16_t units[2]; size_t length=1;
        if(point>0xFFFF) {
            units[0]=(uint16_t)get_hi_surrogate(point);
            units[1]=(uint16_t)get_lo_surrogate(point); length=2;
        } else units[0]=(uint16_t)point;
        ls_string input={units,length,NULL};
        for(int upper=0;upper<2;upper++) {
            ls_string text=ls_string_case(input,upper!=0);
            assert(text.length<256); fputc((int)text.length,stdout);
            for(size_t i=0;i<text.length;i++) {
                fputc(text.data[i]&255,stdout); fputc(text.data[i]>>8,stdout);
            }
            ls_string_release(text);
        }
    }
    ls_native_collect_cycles(); assert(ls_native_owned_objects()==0);
    return ferror(stdout)?1:0;
}

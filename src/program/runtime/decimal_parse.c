/* libregexp's companion atod rounds a 38-digit decimal prefix. A JSON value
   may place a distinguishing digit arbitrarily far past that prefix. Retain
   a sticky suffix and compare the original decimal with the next binary64
   midpoint exactly. Truncation is downward and its relative error is <1e-37,
   so a correctly rounded prefix needs at most one upward representable step.
   Normal numbers of at most 38 significant digits keep the library fast path.

   A binary64 midpoint is m*2^q with at most 54 bits in m, -1075<=q<=970.
   Its exact terminating decimal has fewer than 800 digits. Base-1e9 limbs
   and fixed scratch suffice independently of the source token's length. */
typedef struct { uint32_t limbs[96]; size_t length; } ls_decimal_integer;
static LS_NATIVE_UNUSED void ls_decimal_multiply(ls_decimal_integer *value,uint32_t factor) {
    uint64_t carry=0;
    for(size_t i=0;i<value->length;i++) {
        uint64_t product=(uint64_t)value->limbs[i]*factor+carry;
        value->limbs[i]=(uint32_t)(product%UINT32_C(1000000000)); carry=product/UINT32_C(1000000000);
    }
    while(carry) {
        assert(value->length<96);
        value->limbs[value->length++]=(uint32_t)(carry%UINT32_C(1000000000));
        carry/=UINT32_C(1000000000);
    }
}
static LS_NATIVE_UNUSED int ls_decimal_midpoint_compare(const char *first,const char *end,int64_t exponent,uint64_t lower) {
    unsigned encoded=(unsigned)(lower>>52);
    uint64_t mantissa=lower&UINT64_C(0x000fffffffffffff);
    if(encoded) mantissa|=UINT64_C(0x0010000000000000);
    int power=encoded?(int)encoded-1023-53:-1075;
    mantissa=2*mantissa+1;
    ls_decimal_integer value={{0},0};
    do { value.limbs[value.length++]=(uint32_t)(mantissa%UINT32_C(1000000000)); mantissa/=UINT32_C(1000000000); } while(mantissa);
    int left=power<0?-power:power;
    while(left) {
        int chunk=left<(power<0?13:29)?left:(power<0?13:29);
        uint32_t factor=1;
        for(int i=0;i<chunk;i++) factor*=power<0?5:2;
        ls_decimal_multiply(&value,factor); left-=chunk;
    }
    char digits[864]; size_t count=(size_t)snprintf(digits,sizeof digits,"%" PRIu32,value.limbs[value.length-1]);
    for(size_t i=value.length-1;i>0;i--) count+=(size_t)snprintf(digits+count,sizeof digits-count,"%09" PRIu32,value.limbs[i-1]);
    int64_t midpoint_exponent=(int64_t)count+(power<0?power:0);
    if(exponent!=midpoint_exponent) return exponent<midpoint_exponent?-1:1;
    size_t index=0;
    for(const char *at=first;at<end;at++) {
        if(*at=='.') continue;
        char other=index<count?digits[index]:'0';
        if(*at!=other) return *at<other?-1:1;
        index++;
    }
    for(;index<count;index++) if(digits[index]!='0') return -1;
    return 0;
}
static LS_NATIVE_UNUSED double ls_decimal_parse(const char *text,size_t length) {
    const char *end=text+length,*at=text,*first=NULL,*mantissa_end;
    bool negative=*at=='-',dot=false,discarded=false;
    if(negative) at++;
    int64_t integer_digits=0,leading=0; size_t kept=0;
    char prefix[64];
    for(;at<end && *at!='e' && *at!='E';at++) {
        if(*at=='.') { dot=true; continue; }
        if(!dot) integer_digits++;
        if(!first && *at=='0') { leading++; continue; }
        if(!first) first=at;
        if(kept<38) prefix[kept++]=*at;
        else discarded|=*at!='0';
    }
    mantissa_end=at;
    if(!first) return negative?-0.0:0.0;
    int64_t scale=0; bool exponent_negative=false;
    if(at<end) {
        at++;
        if(at<end && (*at=='-' || *at=='+')) { exponent_negative=*at=='-'; at++; }
        for(;at<end;at++) {
            if(scale<INT64_C(1099511627776)) scale=scale*10+(*at-'0');
        }
        if(exponent_negative) scale=-scale;
    }
    int64_t exponent=integer_digits-leading+scale;
    if(exponent>309) return negative?-INFINITY:INFINITY;
    if(exponent< -324) return negative?-0.0:0.0;
    snprintf(prefix+kept,sizeof prefix-kept,"e%d",(int)(exponent-(int64_t)kept));
    JSATODTempMem scratch;
    double number=js_atod(prefix,NULL,10,0,&scratch);
    uint64_t bits; memcpy(&bits,&number,sizeof bits);
    if(discarded && bits<UINT64_C(0x7ff0000000000000)) {
        int comparison=ls_decimal_midpoint_compare(first,mantissa_end,exponent,bits);
        if(comparison>0 || (comparison==0 && (bits&1))) bits++;
    }
    if(negative) bits|=UINT64_C(0x8000000000000000);
    memcpy(&number,&bits,sizeof number); return number;
}

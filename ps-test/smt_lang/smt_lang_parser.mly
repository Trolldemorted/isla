/* generated by Ott 0.30 from: smt_lang.ott */
%{
open Smt_lang_ast
%}

%token VALU_APOSTROPHE_APOSTROPHE_COMMA  (* valu'', *)
%token WRITE_UNDERSCORE_KIND_COLON  (* write_kind: *)
%token READ_UNDERSCORE_KIND_COLON  (* read_kind: *)
%token FIELD_UNDERSCORE_NAME  (* field_name *)
%token VALU_APOSTROPHE_COMMA  (* valu', *)
%token BVI_ONE_TWO_EIGHT  (* bvi128 *)
%token LPAREN_UNDERSCORE  (* (_ *)
%token ADDRESS_COLON  (* address: *)
%token BVI_SIX_FOUR  (* bvi64 *)
%token DECLARECONST  (* DeclareConst *)
%token QUESTIONMARK  (* ? *)
%token VALUUE_COLON  (* valuue: *)
%token BYTES_COLON  (* bytes: *)
%token DEFINECONST  (* DefineConst *)
%token DATA_COLON  (* data: *)
%token SIGNEXTEND  (* signExtend *)
%token VALU_COMMA  (* valu, *)
%token ZEROEXTEND  (* zeroExtend *)
%token BVREDAND  (* bvredand *)
%token WRITEMEM  (* WriteMem *)
%token WRITEREG  (* WriteReg *)
%token BVREDOR  (* bvredor *)
%token EXTRACT  (* extract *)
%token READMEM  (* ReadMem *)
%token READREG  (* ReadReg *)
%token ASSERT  (* Assert *)
%token BITVEC  (* BitVec *)
%token BVASHR  (* bvashr *)
%token BVLSHR  (* bvlshr *)
%token BVNAND  (* bvnand *)
%token BVSDIV  (* bvsdiv *)
%token BVSMOD  (* bvsmod *)
%token BVSREM  (* bvsrem *)
%token BVUDIV  (* bvudiv *)
%token BVUREM  (* bvurem *)
%token BVXNOR  (* bvxnor *)
%token CONCAT  (* concat *)
%token LBRACE  (* { *)
%token LBRACK  (* [ *)
%token LPAREN  (* ( *)
%token POISON  (* poison *)
%token RBRACE  (* } *)
%token RBRACK  (* ] *)
%token RPAREN  (* ) *)
%token STRUCT  (* Struct *)
%token BVADD  (* bvadd *)
%token BVAND  (* bvand *)
%token BVMUL  (* bvmul *)
%token BVNEG  (* bvneg *)
%token BVNOR  (* bvnor *)
%token BVNOT  (* bvnot *)
%token BVSGE  (* bvsge *)
%token BVSGT  (* bvsgt *)
%token BVSHL  (* bvshl *)
%token BVSLE  (* bvsle *)
%token BVSLT  (* bvslt *)
%token BVSUB  (* bvsub *)
%token BVUGE  (* bvuge *)
%token BVUGT  (* bvugt *)
%token BVULE  (* bvule *)
%token BVULT  (* bvult *)
%token BVXOR  (* bvxor *)
%token COLON  (* : *)
%token COMMA  (* , *)
%token FALSE  (* false *)
%token FIELD  (* field *)
%token BOOL  (* Bool *)
%token BVOR  (* bvor *)
%token LIST  (* list *)
%token TRUE  (* true *)
%token UNIT  (* unit *)
%token AND  (* and *)
%token BAR  (* | *)
%token ITE  (* ite *)
%token NEQ  (* neq *)
%token NOT  (* not *)
%token SMT  (* Smt *)
%token VEC  (* vec *)
%token BV  (* bv *)
%token EQ  (* eq *)
%token OR  (* or *)
%token S  (* s *)
%token <string> VU_THREE_TWO  (* metavarroot vu32 *)
%token <int> U_THREE_TWO  (* metavarroot u32 *)
%token <string> U_SIX_FOUR  (* metavarroot u64 *)
%token EOF  (* added by Ott *)

%start <Smt_lang_ast.term> term_start


%%

term_start:
| term = term EOF
    { term }

valu:
| vu32 = VU_THREE_TWO    (* vu32 :: Val_Symbolic *)
    { (*Case 2*) Val_Symbolic(vu32) }
| LPAREN_UNDERSCORE  BVI_SIX_FOUR  RPAREN    (* (_ bvi64 ) :: Val_I64 *)
    { (*Case 2*) Val_I64 }
| LPAREN_UNDERSCORE  BVI_ONE_TWO_EIGHT    (* (_ bvi128 :: Val_I128 *)
    { (*Case 2*) Val_I128 }
| BOOL  LPAREN  bool = bool  RPAREN    (* Bool ( bool ) :: Val_Bool *)
    { (*Case 2*) Val_Bool(bool) }
| BV    (* bv :: Val_Bits *)
    { (*Case 2*) Val_Bits }
| S    (* s :: Val_String *)
    { (*Case 2*) Val_String }
| LPAREN_UNDERSCORE  UNIT  RPAREN    (* (_ unit ) :: Val_Unit *)
    { (*Case 2*) Val_Unit }
| LPAREN_UNDERSCORE  VEC  LBRACE  valu0 = separated_list(COMMA,valu)  RBRACE  RPAREN    (* (_ vec { valu1 , .. , valuk } ) :: Val_Vector *)
    { (*Case 2*) Val_Vector(valu0) }
| LPAREN_UNDERSCORE  LIST  LBRACE  valu0 = separated_list(COMMA,valu)  RBRACE  RPAREN    (* (_ list { valu1 , .. , valuk } ) :: Val_List *)
    { (*Case 2*) Val_List(valu0) }
| STRUCT  LPAREN  LBRACE  u320_valu0 = separated_list(COMMA,tuple3(U_THREE_TWO,COLON,valu))  RBRACE  RPAREN    (* Struct ( { u321 : valu1 , .. , u32k : valuk } ) :: Val_Struct *)
    { (*Case 2*) Val_Struct(List.map (function (u320,(),valu0) -> (u320,valu0)) u320_valu0) }
| LPAREN_UNDERSCORE  POISON  RPAREN    (* (_ poison ) :: Val_Poison *)
    { (*Case 2*) Val_Poison }

ty:
| BOOL    (* Bool :: Ty_Bool *)
    { (*Case 2*) Ty_Bool }
| BITVEC  LPAREN  u32 = U_THREE_TWO  RPAREN    (* BitVec ( u32 ) :: Ty_BitVec *)
    { (*Case 2*) Ty_BitVec(u32) }

bool:
| TRUE    (* true :: True *)
    { (*Case 2*) True }
| FALSE    (* false :: False *)
    { (*Case 2*) False }

exp:
| vu32 = VU_THREE_TWO    (* vu32 :: Var *)
    { (*Case 2*) Var(vu32) }
| BV    (* bv :: Bits *)
    { (*Case 2*) Bits }
| QUESTIONMARK  u64 = U_SIX_FOUR  u32 = U_THREE_TWO    (* ? u64 u32 :: Bits64 *)
    { (*Case 2*) Bits64(u64,u32) }
| bool = bool    (* bool :: Bool *)
    { (*Case 2*) Bool(bool) }
| LPAREN  EQ  exp = exp  exp_prime = exp  RPAREN    (* ( eq exp exp' ) :: Eq *)
    { (*Case 2*) Eq(exp,exp_prime) }
| LPAREN  NEQ  exp = exp  exp_prime = exp  RPAREN    (* ( neq exp exp' ) :: Neq *)
    { (*Case 2*) Neq(exp,exp_prime) }
| LPAREN  AND  exp = exp  exp_prime = exp  RPAREN    (* ( and exp exp' ) :: And *)
    { (*Case 2*) And(exp,exp_prime) }
| LPAREN  OR  exp = exp  exp_prime = exp  RPAREN    (* ( or exp exp' ) :: Or *)
    { (*Case 2*) Or(exp,exp_prime) }
| LPAREN  NOT  exp = exp  RPAREN    (* ( not exp ) :: Not *)
    { (*Case 2*) Not(exp) }
| LPAREN  BVNOT  exp = exp  RPAREN    (* ( bvnot exp ) :: Bvnot *)
    { (*Case 2*) Bvnot(exp) }
| LPAREN  BVREDAND  exp = exp  RPAREN    (* ( bvredand exp ) :: Bvredand *)
    { (*Case 2*) Bvredand(exp) }
| LPAREN  BVREDOR  exp = exp  RPAREN    (* ( bvredor exp ) :: Bvredor *)
    { (*Case 2*) Bvredor(exp) }
| LPAREN  BVAND  exp = exp  exp_prime = exp  RPAREN    (* ( bvand exp exp' ) :: Bvand *)
    { (*Case 2*) Bvand(exp,exp_prime) }
| LPAREN  BVOR  exp = exp  exp_prime = exp  RPAREN    (* ( bvor exp exp' ) :: Bvor *)
    { (*Case 2*) Bvor(exp,exp_prime) }
| LPAREN  BVXOR  exp = exp  exp_prime = exp  RPAREN    (* ( bvxor exp exp' ) :: Bvxor *)
    { (*Case 2*) Bvxor(exp,exp_prime) }
| LPAREN  BVNAND  exp = exp  exp_prime = exp  RPAREN    (* ( bvnand exp exp' ) :: Bvnand *)
    { (*Case 2*) Bvnand(exp,exp_prime) }
| LPAREN  BVNOR  exp = exp  exp_prime = exp  RPAREN    (* ( bvnor exp exp' ) :: Bvnor *)
    { (*Case 2*) Bvnor(exp,exp_prime) }
| LPAREN  BVXNOR  exp = exp  exp_prime = exp  RPAREN    (* ( bvxnor exp exp' ) :: Bvxnor *)
    { (*Case 2*) Bvxnor(exp,exp_prime) }
| LPAREN  BVNEG  exp = exp  RPAREN    (* ( bvneg exp ) :: Bvneg *)
    { (*Case 2*) Bvneg(exp) }
| LPAREN  BVADD  exp = exp  exp_prime = exp  RPAREN    (* ( bvadd exp exp' ) :: Bvadd *)
    { (*Case 2*) Bvadd(exp,exp_prime) }
| LPAREN  BVSUB  exp = exp  exp_prime = exp  RPAREN    (* ( bvsub exp exp' ) :: Bvsub *)
    { (*Case 2*) Bvsub(exp,exp_prime) }
| LPAREN  BVMUL  exp = exp  exp_prime = exp  RPAREN    (* ( bvmul exp exp' ) :: Bvmul *)
    { (*Case 2*) Bvmul(exp,exp_prime) }
| LPAREN  BVUDIV  exp = exp  exp_prime = exp  RPAREN    (* ( bvudiv exp exp' ) :: Bvudiv *)
    { (*Case 2*) Bvudiv(exp,exp_prime) }
| LPAREN  BVSDIV  exp = exp  exp_prime = exp  RPAREN    (* ( bvsdiv exp exp' ) :: Bvsdiv *)
    { (*Case 2*) Bvsdiv(exp,exp_prime) }
| LPAREN  BVUREM  exp = exp  exp_prime = exp  RPAREN    (* ( bvurem exp exp' ) :: Bvurem *)
    { (*Case 2*) Bvurem(exp,exp_prime) }
| LPAREN  BVSREM  exp = exp  exp_prime = exp  RPAREN    (* ( bvsrem exp exp' ) :: Bvsrem *)
    { (*Case 2*) Bvsrem(exp,exp_prime) }
| LPAREN  BVSMOD  exp = exp  exp_prime = exp  RPAREN    (* ( bvsmod exp exp' ) :: Bvsmod *)
    { (*Case 2*) Bvsmod(exp,exp_prime) }
| LPAREN  BVULT  exp = exp  exp_prime = exp  RPAREN    (* ( bvult exp exp' ) :: Bvult *)
    { (*Case 2*) Bvult(exp,exp_prime) }
| LPAREN  BVSLT  exp = exp  exp_prime = exp  RPAREN    (* ( bvslt exp exp' ) :: Bvslt *)
    { (*Case 2*) Bvslt(exp,exp_prime) }
| LPAREN  BVULE  exp = exp  exp_prime = exp  RPAREN    (* ( bvule exp exp' ) :: Bvule *)
    { (*Case 2*) Bvule(exp,exp_prime) }
| LPAREN  BVSLE  exp = exp  exp_prime = exp  RPAREN    (* ( bvsle exp exp' ) :: Bvsle *)
    { (*Case 2*) Bvsle(exp,exp_prime) }
| LPAREN  BVUGE  exp = exp  exp_prime = exp  RPAREN    (* ( bvuge exp exp' ) :: Bvuge *)
    { (*Case 2*) Bvuge(exp,exp_prime) }
| LPAREN  BVSGE  exp = exp  exp_prime = exp  RPAREN    (* ( bvsge exp exp' ) :: Bvsge *)
    { (*Case 2*) Bvsge(exp,exp_prime) }
| LPAREN  BVUGT  exp = exp  exp_prime = exp  RPAREN    (* ( bvugt exp exp' ) :: Bvugt *)
    { (*Case 2*) Bvugt(exp,exp_prime) }
| LPAREN  BVSGT  exp = exp  exp_prime = exp  RPAREN    (* ( bvsgt exp exp' ) :: Bvsgt *)
    { (*Case 2*) Bvsgt(exp,exp_prime) }
| LPAREN  LPAREN_UNDERSCORE  EXTRACT  u32 = U_THREE_TWO  u32_prime = U_THREE_TWO  RPAREN  exp_prime_prime = exp  RPAREN    (* ( (_ extract u32 u32' ) exp'' ) :: Extract *)
    { (*Case 2*) Extract(u32,u32_prime,exp_prime_prime) }
| LPAREN  LPAREN_UNDERSCORE  ZEROEXTEND  u32 = U_THREE_TWO  RPAREN  exp_prime = exp  RPAREN    (* ( (_ zeroExtend u32 ) exp' ) :: ZeroExtend *)
    { (*Case 2*) ZeroExtend(u32,exp_prime) }
| LPAREN  LPAREN_UNDERSCORE  SIGNEXTEND  u32 = U_THREE_TWO  RPAREN  exp_prime = exp  RPAREN    (* ( (_ signExtend u32 ) exp' ) :: SignExtend *)
    { (*Case 2*) SignExtend(u32,exp_prime) }
| LPAREN  BVSHL  exp = exp  exp_prime = exp  RPAREN    (* ( bvshl exp exp' ) :: Bvshl *)
    { (*Case 2*) Bvshl(exp,exp_prime) }
| LPAREN  BVLSHR  exp = exp  exp_prime = exp  RPAREN    (* ( bvlshr exp exp' ) :: Bvlshr *)
    { (*Case 2*) Bvlshr(exp,exp_prime) }
| LPAREN  BVASHR  exp = exp  exp_prime = exp  RPAREN    (* ( bvashr exp exp' ) :: Bvashr *)
    { (*Case 2*) Bvashr(exp,exp_prime) }
| LPAREN  CONCAT  exp = exp  exp_prime = exp  RPAREN    (* ( concat exp exp' ) :: Concat *)
    { (*Case 2*) Concat(exp,exp_prime) }
| LPAREN  ITE  exp = exp  exp_prime = exp  exp_prime_prime = exp  RPAREN    (* ( ite exp exp' exp'' ) :: Ite *)
    { (*Case 2*) Ite(exp,exp_prime,exp_prime_prime) }

def:
| DECLARECONST  LPAREN  u32 = U_THREE_TWO  COMMA  ty = ty  RPAREN    (* DeclareConst ( u32 , ty ) :: DeclareConst *)
    { (*Case 2*) DeclareConst(u32,ty) }
| DEFINECONST  LPAREN  u32 = U_THREE_TWO  COMMA  exp = exp  RPAREN    (* DefineConst ( u32 , exp ) :: DefineConst *)
    { (*Case 2*) DefineConst(u32,exp) }
| ASSERT  LPAREN  exp = exp  RPAREN    (* Assert ( exp ) :: Assert *)
    { (*Case 2*) Assert(exp) }

accessor:
| LPAREN_UNDERSCORE  FIELD  BAR  FIELD_UNDERSCORE_NAME  BAR  RPAREN    (* (_ field | field_name | ) :: Field *)
    { (*Case 2*) Field }

event:
| SMT  LPAREN  def = def  RPAREN    (* Smt ( def ) :: Smt *)
    { (*Case 2*) Smt(def) }
| READREG  LPAREN  u32 = U_THREE_TWO  COMMA  LBRACK  accessor0 = separated_list(COMMA,accessor)  RBRACK  COMMA  valu = valu  RPAREN    (* ReadReg ( u32 , [ accessor1 , .. , accessork ] , valu ) :: ReadReg *)
    { (*Case 2*) ReadReg(u32,accessor0,valu) }
| WRITEREG  LPAREN  u32 = U_THREE_TWO  COMMA  valu = valu  RPAREN    (* WriteReg ( u32 , valu ) :: WriteReg *)
    { (*Case 2*) WriteReg(u32,valu) }
| READMEM  LBRACE  VALUUE_COLON  u32 = U_THREE_TWO  COMMA  READ_UNDERSCORE_KIND_COLON  VALU_COMMA  ADDRESS_COLON  VALU_APOSTROPHE_COMMA  BYTES_COLON  u32_prime = U_THREE_TWO  RBRACE    (* ReadMem { valuue: u32 , read_kind: valu, address: valu', bytes: u32' } :: ReadMem *)
    { (*Case 2*) ReadMem(u32,u32_prime) }
| WRITEMEM  LBRACE  VALUUE_COLON  u32 = U_THREE_TWO  COMMA  WRITE_UNDERSCORE_KIND_COLON  VALU_COMMA  ADDRESS_COLON  VALU_APOSTROPHE_COMMA  DATA_COLON  VALU_APOSTROPHE_APOSTROPHE_COMMA  BYTES_COLON  u32_prime = U_THREE_TWO  RBRACE    (* WriteMem { valuue: u32 , write_kind: valu, address: valu', data: valu'', bytes: u32' } :: WriteMem *)
    { (*Case 2*) WriteMem(u32,u32_prime) }

term:
| def = def    (* def :: Def *)
    { (*Case 2*) Def(def) }
| event = event    (* event :: Event *)
    { (*Case 2*) Event(event) }

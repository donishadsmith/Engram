-- https://github.com/pret/pokeemerald/blob/master/src/overworld.c
-- https://github.com/pret/pokeemerald/blob/master/include/global.h
-- https://github.com/pret/pokeemerald/blob/master/src/money.c
-- https://github.com/roytam1/rtoss/blob/master/PokemonHackSourceCode/PokemonMemHack/PokemonMemHackCore.cpp#L947

local money_pointer = read_u32(0x03005D8C) + 0x490
local encryption_key_pointer = read_u32(0x03005D90) + 0xAC
local encryption_key = read_u32(encryption_key_pointer)

local current_money = read_u32(money_pointer) ~ encryption_key
print("Current Money:", current_money)

local new_value = encryption_key ~ 999999
write_u32(money_pointer, new_value)

local updated_money = read_u32(money_pointer) ~ encryption_key
print("Updated Money:", updated_money)

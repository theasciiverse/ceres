-- headless reference: run the original cart with a minimal PICO-8 shim
local level = tonumber(arg[1])
local tasdir = arg[2]
local nframes = tonumber(arg[3] or 400)

local function readall(p) local f=assert(io.open(p,'rb')); local s=f:read('a'); f:close(); return s end
local MAP = readall('cartmap.bin')
local FLAGS = readall('cartflags.bin')
local out = io.write
local seed = 12345
local buttons = {}

-- shim
function flr(x) return math.floor(x) end
function abs(x) return math.abs(x) end
function min(a,b) return math.min(a,b) end
function max(a,b) return math.max(a,b) end
function sin(x) return -math.sin((x or 0)*math.pi*2) end
function cos(x) return math.cos((x or 0)*math.pi*2) end
function rnd(x) seed = (seed * 1103515245 + 12345) % 2147483648; return (seed / 2147483648) * (x or 1) end
function sfx() end function music() end function pal() end function camera() end
function rectfill() end function circfill() end function spr() end function map() end
print = function() end
function btn(i) return buttons[i] == true end
function mget(x,y) return MAP:byte(x + y*128 + 1) end
function fget(t,f) if t>=128 then return false end local b=FLAGS:byte(t+1) return (b >> f) & 1 == 1 end
function add(a,v) a[#a+1]=v end
function del(a,dv) for i=1,#a do if a[i]==dv then table.remove(a,i) return end end end
function count(a) local c=0 for i=1,#a do if a[i]~=nil then c=c+1 end end return c end
function all(a)
  if a==nil or #a==0 then return function() end end
  local i, li = 1
  return function()
    if a[i]==li then i=i+1 end
    while a[i]==nil and i<=#a do i=i+1 end
    li=a[i]
    return a[i]
  end
end
function foreach(a,f) for v in all(a) do f(v) end end

dofile('cart.lua')

-- TAS file
local text = readall(tasdir .. '/TAS' .. level .. '.tas')
local head, body = text:match('^%[(.-)%](.*)$')
local seeds, keys = {}, {}
for s in head:gmatch('[^,]+') do seeds[#seeds+1] = tonumber(s) end
for s in body:gmatch('[^,%s]+') do keys[#keys+1] = tonumber(s) end

_init()
local idx = level - 1
max_djump = idx > 21 and 2 or 1
got_fruit[idx+1] = false
load_room(idx % 8, idx // 8)
local si = 1
for _,o in pairs(objects) do
  if o.type.id == 'balloon' then o.initial_offset = seeds[si] or 0; o.offset = o.initial_offset; si = si + 1
  elseif o.type.id == 'chest' then o.offset = seeds[si] or 0; si = si + 1
  elseif o.type.id == 'platform' then o.rem.x = o.dir * 0.65 end
end

local kf = 0 -- index into keys (file entries), 0 = none yet
for frame = 0, nframes do
  local bits = (show_keys and keys[kf]) or 0
  for b = 0, 5 do buttons[b] = (bits // (1 << b)) % 2 == 1 end
  update = true
  _update()
  _draw()
  local p
  for _,o in pairs(objects) do if o.type.id == 'player' or o.type.id == 'player_spawn' then p = o end end
  if p then
    out(string.format('%d %s %d %.17g %.17g %.17g %.17g %.17g %.17g\n', frame, p.type.id == 'player' and 'P' or 'S', bits, p.x, p.y, p.spd.x, p.spd.y, p.rem.x, p.rem.y))
  else
    out(string.format('%d - %d\n', frame, bits))
  end
  if beat_level then out('BEAT\n') break end
  if died then out('DIED\n') break end
  if show_keys then kf = kf + 1 end
end

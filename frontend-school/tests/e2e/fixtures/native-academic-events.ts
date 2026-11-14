import {createHash} from 'node:crypto';
import {createServer} from 'node:http';
import type {Duplex} from 'node:stream';
import type {TimetableEvent} from '../../../src/lib/stores/timetable-socket';

/** Real native frames for the general academic channel; business authorization is tested by Rust HTTP fixtures. */
export async function startNativeAcademicEvents(){
 const peers=new Map<Duplex,{term:string;connectionId:string}>();let connections=0;
 const server=createServer();
 function frame(socket:Duplex,opcode:number,payload:Buffer){
  if(socket.destroyed||socket.writableEnded)return;if(payload.length>=65536)throw new Error('academic fixture frame exceeds its explicit bound');
  const header=payload.length<126?Buffer.from([0x80|opcode,payload.length]):Buffer.from([0x80|opcode,126,payload.length>>8,payload.length&255]);socket.write(Buffer.concat([header,payload]));
 }
 function send(socket:Duplex,event:TimetableEvent&{seq?:number}){frame(socket,1,Buffer.from(JSON.stringify(event)));}
 server.on('upgrade',(request,socket,head)=>{
  const key=request.headers['sec-websocket-key'],url=new URL(request.url??'/','http://localhost');
  if(typeof key!=='string'||!url.searchParams.get('academicTermId')){socket.destroy();return;}
  socket.write('HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: '+createHash('sha1').update(key+'258EAFA5-E914-47DA-95CA-C5AB0DC85B11').digest('base64')+'\r\n\r\n');
  peers.set(socket,{term:url.searchParams.get('academicTermId')!,connectionId:url.searchParams.get('fixtureConnectionId')??''});connections++;send(socket,{type:'StateSync',payload:{users:[],current_seq:0}});
  let buffered=head;
  function read(data:Buffer){
   buffered=Buffer.concat([buffered,data]);
   while(buffered.length>=2){
    const opcode=buffered[0]&15;let size=buffered[1]&127,offset=2;
    if(size===126){if(buffered.length<4)return;size=buffered.readUInt16BE(2);offset=4;}
    else if(size===127){socket.destroy();return;}
    const masked=Boolean(buffered[1]&128),maskBytes=masked?4:0;if(buffered.length<offset+maskBytes+size)return;
    const mask=masked?buffered.subarray(offset,offset+4):null,payload=Buffer.from(buffered.subarray(offset+maskBytes,offset+maskBytes+size));if(mask)for(let index=0;index<payload.length;index++)payload[index]^=mask[index%4];buffered=buffered.subarray(offset+maskBytes+size);
    if(opcode===8){frame(socket,8,payload);socket.end();return;}if(opcode===9)frame(socket,10,payload);
   }
  }
  socket.on('data',read);socket.on('error',()=>socket.destroy());socket.on('close',()=>peers.delete(socket));if(head.length)read(Buffer.alloc(0));
 });
 await new Promise<void>(resolve=>server.listen(0,'127.0.0.1',resolve));const address=server.address();if(!address||typeof address==='string')throw new Error('missing owned academic relay port');
 return{url:`ws://127.0.0.1:${address.port}`,connections:()=>connections,liveConnectionIds:(term:string)=>[...peers].filter(([socket,selected])=>selected.term===term&&!socket.destroyed&&!socket.writableEnded).map(([,selected])=>selected.connectionId),send:(term:string,event:TimetableEvent&{seq?:number})=>{let sent=0;for(const[socket,selected]of peers)if(selected.term===term&&!socket.destroyed&&!socket.writableEnded){send(socket,event);sent++;}if(!sent)throw new Error('no live native peer in selected academic context');},async close(){for(const socket of peers.keys())socket.destroy();await new Promise<void>((resolve,reject)=>server.close(error=>error?reject(error):resolve()));}};
}

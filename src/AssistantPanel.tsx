import { useEffect, useRef, useState } from 'react';
import { Mic, Send, Sparkles, Square } from 'lucide-react';
import { call } from './api';
import type { AssistantStatus, VoiceStatus } from './types';
import { Notice, errorMessage } from './ui';
type ChatMessage = {id:string;role:'user'|'assistant';text:string;details?:string;interrupted?:boolean};
type Answer = {text:string;brief_text?:string;intent?:string};
type Transcript = {text:string;language?:string;no_speech?:boolean};
const delay = (ms:number) => new Promise<void>(resolve => setTimeout(resolve,ms));
export const aiLabel = (status:AssistantStatus|null) => status?.ready ? 'AI Ready' : status?.state==='disabled' ? 'AI Off' : status?.running || status?.state === 'starting' || status?.state === 'recovering' ? 'Starting AI…' : 'AI Unavailable';
export default function AssistantPanel({status,visible}: {status:AssistantStatus|null;visible:boolean}) {
  const session = useRef(crypto.randomUUID()),epoch = useRef(0),mounted = useRef(true),bottom = useRef<HTMLDivElement>(null);
  const [messages,setMessages] = useState<ChatMessage[]>([]),[question,setQuestion] = useState(''),[busy,setBusy] = useState(false),[error,setError] = useState(''),[voiceActive,setVoiceActive] = useState(false),[phase,setPhase] = useState('Ready to start'),[language,setLanguage] = useState('auto');
  const add = (role:ChatMessage['role'],text:string,details?:string) => {const id=crypto.randomUUID();setMessages(rows=>[...rows,{id,role,text,details}]);return id;};
  const reply = (answer:Answer) => add('assistant',answer.brief_text||answer.text,answer.brief_text&&answer.brief_text!==answer.text?answer.text:undefined);
  async function stopVoice() {epoch.current++;setVoiceActive(false);setPhase('Conversation ended');await call('voice_stop').catch(() => {});}
  useEffect(() => {if(!visible && voiceActive) void stopVoice();},[visible]);
  useEffect(() => {if(status?.enabled===false&&voiceActive) void stopVoice();},[status?.enabled,voiceActive]);
  useEffect(() => {mounted.current = true;return () => {mounted.current = false;epoch.current++;void call('voice_stop').catch(() => {});};},[]);
  useEffect(() => {if(visible) bottom.current?.scrollIntoView({block:'nearest'});},[messages,busy,visible]);
  useEffect(() => {if(voiceActive&&status?.voice_interrupted) setPhase(current=>current.startsWith('Speaking')?'Listening…':current);},[status?.voice_interrupted,voiceActive]);
  async function ask(text:string) {
    setBusy(true);setError('');add('user',text);
    try {const answer = await call<Answer>('assistant_ask',{question:text,session_id:session.current});reply(answer);}
    catch(e) {setError(errorMessage(e));} finally {setBusy(false);}
  }
  async function newConversation() {setError('');try {await call('assistant_new_conversation',{session_id:session.current});session.current = crypto.randomUUID();setMessages([]);} catch(e) {setError(errorMessage(e));}}
  async function startVoice() {
    const run = ++epoch.current;setVoiceActive(true);setError('');setPhase('Starting voice…');
    const active = () => mounted.current && epoch.current === run;
    async function waitReady() {
      const deadline = Date.now()+(status?.macos_controls?1200000:150000);
      while(active() && Date.now() < deadline) {const voice = await call<VoiceStatus>('voice_status');if(voice.ready) return;if(!voice.configured || !voice.active) throw new Error(voice.message || 'Voice unavailable.');await delay(500);}
      if(active()) throw new Error('Voice took too long to load. Please try again.');
    }
    try {
      await call('voice_start');await waitReady();
      let pending:Transcript|null=null;
      while(active()) {
        let transcript:Transcript;
        try {setPhase('Listening…');transcript = pending || await call('voice_listen',{language});pending=null;}
        catch(e) {if(!active()) break;const voice = await call<VoiceStatus>('voice_status');if(voice.active && !voice.ready) {setPhase('Recovering voice…');await waitReady();continue;}throw e;}
        if(!active()) break;if(transcript.no_speech || !transcript.text) continue;
        add('user',transcript.text);setPhase('Thinking…');
        let answer:Answer;
        try {answer = await call('assistant_ask',{question:transcript.text,session_id:session.current});}
        catch(e) {
          if(!active()) break;
          const deadline = Date.now()+90000;let ready = false;
          while(active() && Date.now() < deadline) {const ai = await call<AssistantStatus>('assistant_status');if(ai.ready) {ready = true;break;}setPhase('Recovering AI…');await delay(1000);}
          if(!ready || !active()) {if(!active()) break;throw e;}
          answer = await call('assistant_ask',{question:transcript.text,session_id:session.current});
        }
        if(!active()) break;const replyId=reply(answer);setPhase('Speaking… you can interrupt');
        const playback=await call<{interrupted?:boolean;transcript?:Transcript}>('voice_speak',{text:answer.brief_text||answer.text,input_language:language,language:transcript.language === 'ar' ? 'ar' : transcript.language === 'en' ? 'en' : 'auto'});
        if(!active()) break;
        if(playback.interrupted) {setMessages(rows=>rows.map(m=>m.id===replyId?{...m,interrupted:true}:m));setPhase('Listening…');pending=playback.transcript?.text?playback.transcript:null;}
        else if(answer.intent==='farewell') {setPhase('Conversation ended');break;}
      }
    } catch(e) {if(active()) {setError(errorMessage(e));setPhase('Voice unavailable');}}
    finally {if(active()) {epoch.current++;setVoiceActive(false);await call('voice_stop').catch(() => {});}}
  }
  function submitQuestion() {if(question.trim() && status?.ready && !busy && !voiceActive) {const q = question.trim();setQuestion('');void ask(q);}}
  return <div className="settings-stack"><div className="panel voice-panel"><div><div className="voice-icon"><Mic size={23}/></div><h2>Voice conversation</h2><p>Speak in Arabic or English. You can interrupt the assistant by speaking. Pause when your question is finished.</p></div><div className="voice-controls"><label className="field"><span>Speech language</span><select aria-label="Speech language" value={language} onChange={e => setLanguage(e.target.value)} disabled={voiceActive}><option value="auto">Automatic — Arabic / English</option><option value="ar">Arabic</option><option value="en">English</option></select></label>{voiceActive ? <button className="btn danger" onClick={() => void stopVoice()}><Square size={15}/>End Conversation</button> : <button className="btn primary" onClick={() => void startVoice()} disabled={!status?.ready || busy || !status?.voice_configured}><Mic size={16}/>Start Conversation</button>}<span className="voice-phase" role="status">{status?.voice_configured ? phase : status?.voice_message || 'Voice resources unavailable. Repair the installation.'}</span></div></div><div className="panel chat-panel"><div className="chat-toolbar"><div><span className="assistant-symbol"><Sparkles size={18}/></span><strong>Inventory Assistant</strong><span className={`ai-status ${status?.ready ? '' : 'starting'}`}><i className="status-dot"/>{aiLabel(status)}</span></div><button className="btn small" disabled={busy || voiceActive} onClick={() => void newConversation()}>New conversation</button></div>{!status?.ready && <div className="ai-service-note">{status?.message ?? 'Starting the bundled local model…'}<span>Inventory management remains available while AI loads.</span></div>}<div className="chat-messages" aria-live="polite">{!messages.length && <div className="chat-welcome"><div className="welcome-icon"><Sparkles size={31}/></div><h2>Your local inventory assistant</h2><p>Ask about products, quantities, CR sales, profit or how to use the application.</p><div className="suggestions">{['How many screens do we have?','What is our profit this month?','How do I add a product?','كم عدد الشاشات في المخزون؟'].map(q => <button key={q} disabled={!status?.ready || busy || voiceActive} onClick={() => void ask(q)}>{q}</button>)}</div></div>}{messages.map(m => <div key={m.id} className={`chat-message ${m.role}`}><span>{m.role === 'user' ? 'You' : <Sparkles size={16}/>}</span><div dir="auto">{m.text}{m.interrupted && <small className="chat-interruption">Interrupted — listening to you</small>}{m.details && <details className="chat-details"><summary>More details</summary><div>{m.details}</div></details>}</div></div>)}{busy && <div className="chat-message assistant"><span><Sparkles size={16}/></span><div>Thinking…</div></div>}<div ref={bottom}/></div><div className="chat-compose"><Notice text={error}/><form onSubmit={e => {e.preventDefault();submitQuestion();}}><textarea aria-label="Ask the inventory assistant" rows={2} maxLength={1000} placeholder="Ask a question in English or Arabic…" dir="auto" value={question} disabled={voiceActive} onChange={e => setQuestion(e.target.value)} onKeyDown={e => {if(e.key === 'Enter' && !e.shiftKey) {e.preventDefault();submitQuestion();}}}/><button className="btn primary" aria-label="Send question" disabled={!question.trim() || !status?.ready || busy || voiceActive}><Send size={18}/></button></form><small>Runs entirely on this computer. To change stock, use Stock Entry or Stock Out.</small></div></div></div>;
}
